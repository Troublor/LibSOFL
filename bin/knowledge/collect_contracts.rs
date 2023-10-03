use std::{
    fmt::Write,
    path::Path,
    process,
    sync::{mpsc, Arc},
    thread,
};

use futures::TryFutureExt;
use hashbrown::HashMap;
use indicatif::{MultiProgress, ProgressBar, ProgressState, ProgressStyle};
use libsofl::{
    config::flags::SoflConfig,
    engine::{
        inspectors::call_extract::CallExtractInspector,
        providers::{BcProvider, BcProviderBuilder},
        state::{env::TransitionSpecBuilder, BcState, BcStateBuilder},
    },
    error::SoflError,
    knowledge,
};
use reth_interfaces::RethError;
use reth_primitives::{Address, BlockHashOrNumber, TxHash};
use sea_orm::{
    sea_query::OnConflict, ColumnTrait, Database, DatabaseConnection,
    EntityTrait, QueryFilter, QueryOrder,
};
use tokio::runtime::Runtime;
use tracing::{debug, error, info};

const METADATA_KEY: &str = "last_call_extract_block";

pub async fn collect_contracts(from: u32, to: u32, jobs: u32, cfg: SoflConfig) {
    // blockchain provider
    info!(datadir = %cfg.reth.datadir, "creating blockchain provider");
    let datadir = Path::new(cfg.reth.datadir.as_str());
    let provider = match BcProviderBuilder::with_mainnet_reth_db(datadir) {
        Err(e) => {
            error!(err = %e, "failed to create provider based on reth db");
            process::exit(1);
        }
        Ok(provider) => provider,
    };
    let provider = Arc::new(provider);

    // database connection
    info!(database = %cfg.database.url, "connecting to database");
    let db = Database::connect(cfg.database.url).await.unwrap();

    // reload the last analyzed block number
    let db = Arc::new(db);
    let last_block =
        get_last_block(&db).await.expect("failed to get last block") + 1;
    if last_block > to {
        info!(last_block, "recap from last block");
    }
    let from = from.max(last_block);
    info!(from, to, "start collecting contracts");

    if to <= from {
        info!(from, to, "no blocks to analyze");
        return;
    }

    let (task_ch_tx, task_ch_rx) = mpsc::sync_channel::<
        mpsc::Receiver<(
            u32,
            Result<
                (Vec<(TxHash, Address)>, Vec<Address>),
                SoflError<RethError>,
            >,
        )>,
    >(jobs as usize);

    // progress bar
    let m = MultiProgress::new();
    let pb = m.add(ProgressBar::new((to - from) as u64));
    pb.set_style(
        ProgressStyle::with_template("{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {current}/{last} ({percent}%), ETA: {eta}")
        .unwrap()
        .with_key("eta", |state: &ProgressState, w: &mut dyn Write| write!(w, "{:.1}s", state.eta().as_secs_f64()).unwrap())
        .with_key("current", move |state: &ProgressState, w: &mut dyn Write| write!(w, "{}", state.pos() + from as u64).unwrap())
        .with_key("last", move |_: &ProgressState, w: &mut dyn Write| write!(w, "{}", to-1).unwrap()),
    );

    // task schedule thread
    let schedule_thread = thread::spawn(move || {
        for block in from..to {
            let p = provider.clone();
            let (analysis_ch_tx, analysis_ch_rx) = mpsc::channel();
            task_ch_tx
                .send(analysis_ch_rx)
                .expect("failed to send task channel");
            thread::spawn(move || {
                let r = analyze_block(p, block as u64);
                analysis_ch_tx
                    .send((block, r))
                    .expect("failed to send analysis result");
            });
        }
        drop(task_ch_tx);
    });

    // task result thread
    let result_thread = thread::spawn(move || {
        let mut cache = InMemoryCache::new(db.clone());
        let runtime = Runtime::new().unwrap();
        while let Ok(analysis_ch_rx) = task_ch_rx.recv() {
            let (block, r) = analysis_ch_rx
                .recv()
                .expect("failed to recv analysis result");
            let (created, invoked) = match r {
                Ok(v) => v,
                Err(e) => {
                    error!(err = %e, block, "failed to analyze block");
                    continue;
                }
            };
            let async_task = cache
                .complete_block(block, created, invoked)
                .and_then(|_| save_last_block(db.clone(), block));
            match runtime.block_on(async_task) {
                Ok(_) => {
                    debug!(block, "collected block");
                }
                Err(e) => {
                    error!(err = %e, block, "failed to collect block");
                    continue;
                }
            }

            pb.inc(1);
        }
        let async_task = cache.save_to_db(to - 1);
        runtime
            .block_on(async_task)
            .expect("failed to save cache to db");

        pb.finish();
    });

    schedule_thread
        .join()
        .expect("failed to join schedule thread");
    result_thread.join().expect("failed to join result thread");

    info!(from, to, "finish collecting contracts");
}

/// Analyze a block range, collect contracts and its invocations.
/// This function should be thread safe.
async fn _analyze<P: BcProvider>(
    provider: Arc<P>,
    db: Arc<DatabaseConnection>,
    from: u32, // included
    to: u32,   // excluded
) {
    let mut cache = InMemoryCache::new(db.clone());

    for block in from..to {
        info!(block, "analyzing block");
        let (created, invoked) =
            match analyze_block(provider.clone(), block as u64) {
                Ok(v) => v,
                Err(e) => {
                    error!(block, err = %e, "failed to analyze block");
                    continue;
                }
            };
        match cache.complete_block(block, created, invoked).await {
            Ok(_) => {
                // save last block
                save_last_block(db.clone(), block)
                    .await
                    .expect("failed to save last block");
            }
            Err(e) => {
                error!(block, err = %e, "failed to save block");
                continue;
            }
        };
    }
    cache
        .save_to_db(to - 1)
        .await
        .expect("failed to save cache to db");
}

#[derive(Debug)]
struct InMemoryCache {
    db: Arc<DatabaseConnection>,
    /// contract continuously invoked in the current block => the first block it was invoked
    pub invoked: HashMap<Address, u32>,
}

impl InMemoryCache {
    pub fn new(db: Arc<DatabaseConnection>) -> Self {
        Self {
            db,
            invoked: HashMap::new(),
        }
    }
}

impl InMemoryCache {
    /// Update the cache with analysis results of a block:
    /// - the list of created contracts in the block
    /// - the list of contracts that are invoked in the block
    /// Created contracts will be saved to db.
    /// Contracts not invoked in this block will be saved to db.
    /// Contracts continuously invoked in this block will still be kept in cache.
    pub async fn complete_block(
        &mut self,
        block: u32,
        created: Vec<(TxHash, Address)>,
        invoked: Vec<Address>,
    ) -> Result<(), sea_orm::DbErr> {
        // insert created contracts
        save_created(self.db.clone(), created).await?;
        // find contracts that are not invoked in this block
        let mut new_invoked = HashMap::new();
        for addr in invoked {
            let first_block = self.invoked.remove(&addr);
            let first_block = match first_block {
                Some(first_block) => {
                    // continuously invoked contract
                    first_block
                }
                None => {
                    // new invoked contract
                    block
                }
            };
            new_invoked.insert(addr, first_block);
        }
        // the rest of old_invoked are not invoked in this block
        self.save_to_db(block - 1).await?;
        // save new_invoked
        self.invoked = new_invoked;
        Ok(())
    }

    pub async fn save_to_db(
        &mut self,
        to_block: u32,
    ) -> Result<(), sea_orm::DbErr> {
        let invocations: Vec<
            knowledge::contract::entities::invocation::ActiveModel,
        > = self
            .invoked
            .iter()
            .map(|(addr, first_block)| {
                knowledge::contract::entities::invocation::ActiveModel {
                    contract: sea_orm::ActiveValue::Set((*addr).into()),
                    from_block: sea_orm::ActiveValue::Set(*first_block),
                    to_block: sea_orm::ActiveValue::Set(to_block),
                }
            })
            .collect();
        if !invocations.is_empty() {
            // save to db
            knowledge::contract::entities::invocation::Entity::insert_many(
                invocations,
            )
            .exec(self.db.as_ref())
            .await?;
        }
        Ok(())
    }
}

/// Save created contracts to db: (Creation Tx, Contract Address)
async fn save_created(
    db: Arc<DatabaseConnection>,
    created: Vec<(TxHash, Address)>,
) -> Result<(), sea_orm::DbErr> {
    if created.is_empty() {
        return Ok(());
    }

    for (tx, addr) in created.into_iter() {
        // check if the contract exists or not
        let last_creation =
            knowledge::contract::entities::creation::Entity::find()
                .filter(
                    knowledge::contract::entities::creation::Column::Contract
                        .eq(knowledge::contract::entities::Address::from(addr)),
                )
                .order_by_desc(
                    knowledge::contract::entities::creation::Column::Index,
                )
                .one(db.as_ref())
                .await?;
        let index = match last_creation {
            Some(last_creation) => last_creation.index + 1,
            None => {
                // insert contract to contract table
                let contract =
                    knowledge::contract::entities::contract::ActiveModel {
                        address: sea_orm::ActiveValue::Set(addr.into()),
                    };
                knowledge::contract::entities::contract::Entity::insert(
                    contract,
                )
                .exec(db.as_ref())
                .await?;
                0
            }
        };
        // insert creation
        let creation = knowledge::contract::entities::creation::ActiveModel {
            contract: sea_orm::ActiveValue::Set(addr.into()),
            create_tx: sea_orm::ActiveValue::Set(tx.into()),
            index: sea_orm::ActiveValue::Set(index),
        };
        knowledge::contract::entities::creation::Entity::insert(creation)
            .exec(db.as_ref())
            .await?;
    }
    Ok(())
}

/// Get the last block number that has been analyzed.
async fn get_last_block(
    db: &DatabaseConnection,
) -> Result<u32, sea_orm::DbErr> {
    let metadata = knowledge::entities::metadata::Entity::find_by_id(
        METADATA_KEY.to_string(),
    )
    .one(db)
    .await?;
    let block = metadata
        .map(|v| {
            v.try_decode::<u32>()
                .expect("failed to decode metadata value")
        })
        .unwrap_or(0);
    Ok(block)
}

/// Save the last block number that has been analyzed.
async fn save_last_block(
    db: Arc<DatabaseConnection>,
    block: u32,
) -> Result<(), sea_orm::DbErr> {
    let metadata: knowledge::entities::metadata::ActiveModel =
        (METADATA_KEY.to_string(), block).into();
    let _ = knowledge::entities::metadata::Entity::insert(metadata)
        .on_conflict(
            OnConflict::column(knowledge::entities::metadata::Column::Key)
                .update_column(knowledge::entities::metadata::Column::Value)
                .to_owned(),
        )
        .exec(db.as_ref())
        .await?;
    Ok(())
}

type Calls = (Vec<(TxHash, Address)>, Vec<Address>);

/// Analyze a block and return a list of contract addresses that are involved in the block.
fn analyze_block<P: BcProvider>(
    provider: Arc<P>,
    block: impl Into<BlockHashOrNumber>,
) -> Result<Calls, SoflError<reth_interfaces::RethError>> {
    let block = block.into();
    let txs = provider.transactions_by_block(block)?;
    let txs = match txs {
        Some(txs) => txs,
        None => return Ok((vec![], vec![])),
    };
    let block_number = match block {
        BlockHashOrNumber::Hash(hash) => {
            provider.block_number(hash)?.ok_or_else(|| {
                SoflError::Custom("failed to get block number".to_string())
            })?
        }
        BlockHashOrNumber::Number(n) => n,
    };
    let state_reader =
        provider.state_by_block_number_or_tag(block_number.into())?;
    let mut called = Vec::new();
    let mut created = Vec::new();
    let mut state = BcStateBuilder::fork_at(provider.as_ref(), block)?;
    for tx in txs.into_iter() {
        let mut insp = CallExtractInspector::default();
        let hash = tx.hash();
        let spec = TransitionSpecBuilder::default()
            .at_block(provider.as_ref(), block)
            .append_signed_tx(tx.clone())
            .build();
        let _ = BcState::transit(&mut state, spec, &mut insp)?;
        let mut called_contracts: Vec<Address> = insp
            .invocations
            .iter()
            .map(|i| i.context.address)
            .filter(|addr| {
                // we are only interested in contracts
                state_reader
                    .account_code(*addr)
                    .expect("failed to get account code")
                    .is_some()
            })
            .collect();
        called_contracts.dedup();
        called.extend(called_contracts);
        let created_contracts: Vec<(TxHash, Address)> = insp
            .creations
            .iter()
            .map(|c| c.contract)
            .filter(|c| c.is_some())
            .map(|c| (hash, c.unwrap()))
            .collect();
        created.extend(created_contracts);
    }
    Ok((created, called))
}

#[cfg(test)]
mod tests_with_db {
    use std::sync::Arc;

    use libsofl::{
        engine::providers::BcProviderBuilder,
        knowledge,
        utils::{
            conversion::{Convert, ToPrimitive},
            testing::get_testing_db,
        },
    };
    use reth_primitives::{Address, TxHash};
    use sea_orm::EntityTrait;

    use crate::collect_contracts::_analyze;

    #[tokio::test]
    async fn test_one_block_call() {
        let provider = BcProviderBuilder::default_db().unwrap();
        let provider = Arc::new(provider);
        let db = get_testing_db().await;
        let db = Arc::new(db);

        // add contract
        let contract0: Address =
            ToPrimitive::cvt("0xc083e9947cf02b8ffc7d3090ae9aea72df98fd47");
        let _ =
            knowledge::contract::entities::contract::Entity::insert_many(vec![
                knowledge::contract::entities::contract::ActiveModel {
                    address: sea_orm::ActiveValue::Set(contract0.into()),
                },
            ])
            .exec(db.as_ref())
            .await
            .unwrap();

        _analyze(provider.clone(), db.clone(), 1000000, 1000001).await;
        let r = knowledge::contract::entities::contract::Entity::find()
            .all(db.as_ref())
            .await
            .unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].address, contract0.into());
        let r = knowledge::contract::entities::invocation::Entity::find()
            .all(db.as_ref())
            .await
            .unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].contract, contract0.into());
        assert_eq!(r[0].from_block, 1000000);
        assert_eq!(r[0].to_block, 1000000);
    }

    #[tokio::test]
    async fn test_one_block_create() {
        let provider = BcProviderBuilder::default_db().unwrap();
        let provider = Arc::new(provider);
        let db = get_testing_db().await;
        let db = Arc::new(db);

        _analyze(provider.clone(), db.clone(), 852349, 852350).await;

        let r = knowledge::contract::entities::contract::Entity::find()
            .find_with_related(knowledge::contract::entities::creation::Entity)
            .all(db.as_ref())
            .await
            .unwrap();
        assert_eq!(r.len(), 1);
        let contract_addr: Address =
            ToPrimitive::cvt("0x9af09991ad63814e53ffc1bccf213ee74027608b");
        let tx: TxHash = ToPrimitive::cvt("0x483a17a77f5acac6819e224bb6a530c4ff0b35c8961ee16f5fa13ac19cb207b7");
        let (contract, creations) = &r[0];
        assert_eq!(contract.address, contract_addr.into());
        assert_eq!(creations[0].create_tx, tx.into());
    }

    #[tokio::test]
    async fn test_continuous_blocks() {
        let provider = BcProviderBuilder::default_db().unwrap();
        let provider = Arc::new(provider);
        let db = get_testing_db().await;
        let db = Arc::new(db);
        let from = 1000000u32;
        let to = 1000011u32;
        let contract0: Address =
            ToPrimitive::cvt("0xc083e9947cf02b8ffc7d3090ae9aea72df98fd47");
        let contract1: Address =
            ToPrimitive::cvt("0xb696c21C287FFA81f9dd79828557231C98D863c9");

        let _ =
            knowledge::contract::entities::contract::Entity::insert_many(vec![
                knowledge::contract::entities::contract::ActiveModel {
                    address: sea_orm::ActiveValue::Set(contract0.into()),
                },
            ])
            .exec(db.as_ref())
            .await
            .unwrap();
        let _ =
            knowledge::contract::entities::contract::Entity::insert_many(vec![
                knowledge::contract::entities::contract::ActiveModel {
                    address: sea_orm::ActiveValue::Set(contract1.into()),
                },
            ])
            .exec(db.as_ref())
            .await
            .unwrap();

        _analyze(provider.clone(), db.clone(), from, to).await;

        let r = knowledge::contract::entities::invocation::Entity::find()
            .all(db.as_ref())
            .await
            .unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].contract, contract0.into());
        assert_eq!(r[0].from_block, from);
        assert_eq!(r[0].to_block, from);
        assert_eq!(r[1].contract, contract1.into());
        assert_eq!(r[1].from_block, to - 1);
        assert_eq!(r[1].to_block, to - 1);
    }
}
