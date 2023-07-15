use hashbrown::HashMap;
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
use reth_primitives::{Address, BlockHashOrNumber, TxHash};
use sea_orm::{
    sea_query::OnConflict, Database, DatabaseConnection, EntityTrait,
};
use tracing::{error, info};
use tracing_subscriber::fmt::SubscriberBuilder;

const METADATA_KEY: &str = "last_call_extract_block";

#[tokio::main]
pub async fn main() {
    let my_subscriber = SubscriberBuilder::default()
        .with_env_filter("debug")
        .finish();
    tracing::subscriber::set_global_default(my_subscriber)
        .expect("setting tracing default failed");

    let target_block: u32 = 1000000;
    let cfg = SoflConfig::load().expect("failed to load config");
    let provider = BcProviderBuilder::default_db()
        .expect("failed to create provider based on reth db");
    let db = Database::connect(cfg.database.url).await.unwrap();
    // reload the last analyzed block number
    let last_block =
        get_last_block(&db).await.expect("failed to get last block") + 1;

    analyze(&provider, &db, last_block, target_block).await;
}

async fn analyze<P: BcProvider>(
    provider: &P,
    db: &DatabaseConnection,
    from: u32,
    to: u32,
) {
    let mut cache = InMemoryCache::new(db);

    for block in from..to {
        info!(block, "analyzing block");
        let (created, invoked) = match analyze_block(provider, block as u64) {
            Ok(v) => v,
            Err(e) => {
                error!(block, err = %e, "failed to analyze block");
                continue;
            }
        };
        match cache.complete_block(block, created, invoked).await {
            Ok(_) => {
                // save last block
                save_last_block(db, block)
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
struct InMemoryCache<'a> {
    db: &'a DatabaseConnection,
    /// contract continuously invoked in the current block => the first block it was invoked
    pub invoked: HashMap<Address, u32>,
}

impl<'a> InMemoryCache<'a> {
    pub fn new(db: &'a DatabaseConnection) -> Self {
        Self {
            db,
            invoked: HashMap::new(),
        }
    }
}

impl<'a> InMemoryCache<'a> {
    pub async fn complete_block(
        &mut self,
        block: u32,
        created: Vec<(TxHash, Address)>,
        invoked: Vec<Address>,
    ) -> Result<(), sea_orm::DbErr> {
        // insert created contracts
        save_created(self.db, created).await?;
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
            .exec(self.db)
            .await?;
        }
        Ok(())
    }
}

async fn save_created(
    db: &DatabaseConnection,
    created: Vec<(TxHash, Address)>,
) -> Result<(), sea_orm::DbErr> {
    if created.is_empty() {
        return Ok(());
    }
    // insert created contracts
    let contracts: Vec<knowledge::contract::entities::contract::ActiveModel> =
        created
            .into_iter()
            .map(|(tx, addr)| {
                knowledge::contract::entities::contract::ActiveModel {
                    address: sea_orm::ActiveValue::Set(addr.into()),
                    create_tx: sea_orm::ActiveValue::Set(tx.into()),
                }
            })
            .collect();
    knowledge::contract::entities::contract::Entity::insert_many(contracts)
        .exec(db)
        .await?;
    Ok(())
}

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

async fn save_last_block(
    db: &DatabaseConnection,
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
        .exec(db)
        .await?;
    Ok(())
}

type Calls = (Vec<(TxHash, Address)>, Vec<Address>);

/// Analyze a block and return a list of contract addresses that are involved in the block.
fn analyze_block<P: BcProvider>(
    provider: &P,
    block: impl Into<BlockHashOrNumber>,
) -> Result<Calls, SoflError<reth_interfaces::Error>> {
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
    let mut state = BcStateBuilder::fork_at(provider, block)?;
    for tx in txs.into_iter() {
        let mut insp = CallExtractInspector::default();
        let hash = tx.hash();
        let spec = TransitionSpecBuilder::default()
            .at_block(provider, block)
            .append_signed_tx(tx)
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
mod tests_nodep {
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

    use crate::analyze;

    #[tokio::test]
    async fn test_one_block_call() {
        let provider = BcProviderBuilder::default_db().unwrap();
        let db = get_testing_db().await;

        // add contract
        let tx: TxHash = ToPrimitive::cvt("0x0");
        let contract0: Address =
            ToPrimitive::cvt("0xc083e9947cf02b8ffc7d3090ae9aea72df98fd47");
        let _ =
            knowledge::contract::entities::contract::Entity::insert_many(vec![
                knowledge::contract::entities::contract::ActiveModel {
                    address: sea_orm::ActiveValue::Set(contract0.into()),
                    create_tx: sea_orm::ActiveValue::Set(tx.into()),
                },
            ])
            .exec(&db)
            .await
            .unwrap();

        analyze(&provider, &db, 1000000, 1000001).await;
        let r = knowledge::contract::entities::contract::Entity::find()
            .all(&db)
            .await
            .unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].address, contract0.into());
        let r = knowledge::contract::entities::invocation::Entity::find()
            .all(&db)
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
        let db = get_testing_db().await;

        analyze(&provider, &db, 852349, 852350).await;

        let r = knowledge::contract::entities::contract::Entity::find()
            .all(&db)
            .await
            .unwrap();
        assert_eq!(r.len(), 1);
        let contract: Address =
            ToPrimitive::cvt("0x9af09991ad63814e53ffc1bccf213ee74027608b");
        let tx: TxHash = ToPrimitive::cvt("0x483a17a77f5acac6819e224bb6a530c4ff0b35c8961ee16f5fa13ac19cb207b7");
        assert_eq!(r[0].address, contract.into());
        assert_eq!(r[0].create_tx, tx.into());
    }

    #[tokio::test]
    async fn test_continuous_blocks() {
        let provider = BcProviderBuilder::default_db().unwrap();
        let db = get_testing_db().await;
        let from = 1000000u32;
        let to = 1000011u32;
        let contract0: Address =
            ToPrimitive::cvt("0xc083e9947cf02b8ffc7d3090ae9aea72df98fd47");
        let contract1: Address =
            ToPrimitive::cvt("0xb696c21C287FFA81f9dd79828557231C98D863c9");
        let tx: TxHash = ToPrimitive::cvt("0x0");

        let _ =
            knowledge::contract::entities::contract::Entity::insert_many(vec![
                knowledge::contract::entities::contract::ActiveModel {
                    address: sea_orm::ActiveValue::Set(contract0.into()),
                    create_tx: sea_orm::ActiveValue::Set(tx.into()),
                },
            ])
            .exec(&db)
            .await
            .unwrap();
        let _ =
            knowledge::contract::entities::contract::Entity::insert_many(vec![
                knowledge::contract::entities::contract::ActiveModel {
                    address: sea_orm::ActiveValue::Set(contract1.into()),
                    create_tx: sea_orm::ActiveValue::Set(tx.into()),
                },
            ])
            .exec(&db)
            .await
            .unwrap();

        analyze(&provider, &db, from, to).await;

        let r = knowledge::contract::entities::invocation::Entity::find()
            .all(&db)
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
