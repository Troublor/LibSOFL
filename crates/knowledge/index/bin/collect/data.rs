use std::collections::{BTreeMap, HashSet};

use libsofl_knowledge_base::entities as base_entities;
use libsofl_knowledge_index::entities;
use libsofl_utils::log::{debug, info};
use sea_orm::{sea_query, EntityTrait};

#[derive(
    Debug, Clone, Eq, PartialEq, Default, serde::Deserialize, serde::Serialize,
)]
struct Progress {
    /// mapping from contract address to the block number from which to the current block that the contract is continuously invoked.
    pub(crate) last_finished_block: u64,
    pub(crate) failed_blocks: Vec<u64>,
}

impl Progress {
    fn new() -> Self {
        Self {
            last_finished_block: 0,
            failed_blocks: Vec::new(),
        }
    }
}

pub(crate) struct DataStore<'a> {
    db: &'a sea_orm::DatabaseConnection,

    progress: Progress,
    pending_invocations: BTreeMap<String, u64>,

    flush_threshold: u64,

    creations_to_insert: Vec<entities::creation::ActiveModel>,
    invocations_to_insert: Vec<entities::invocation::ActiveModel>,
}

const METADATA_KEY: &str = "tx_index_progress";

impl<'a> DataStore<'a> {
    pub async fn new(
        db: &'a sea_orm::DatabaseConnection,
        flush_threshold: u64,
    ) -> Result<Self, sea_orm::DbErr> {
        // load progress
        info!("loading previous progress from database");
        let progress =
            match base_entities::metadata::Entity::find_by_id(METADATA_KEY)
                .one(db)
                .await?
            {
                Some(m) => {
                    let p: Progress = m.try_decode().expect("invalid metadata");
                    info!(
                        last_finished_block = p.last_finished_block,
                        "progress resumed"
                    );
                    p
                }
                None => {
                    info!("no progress found, creating new");
                    Progress::new()
                }
            };
        Ok(Self {
            db,
            progress,
            pending_invocations: BTreeMap::new(),
            flush_threshold,
            creations_to_insert: Vec::new(),
            invocations_to_insert: Vec::new(),
        })
    }
}

impl<'a> DataStore<'a> {
    pub(crate) fn get_last_finished_block(&self) -> u64 {
        self.progress.last_finished_block
    }

    /// Add the invoked contract list to the pending_invocations.
    /// If a previously-pending address is not invoked in the current block (the given address list),
    /// It will be flushed to the underlying database.
    pub(crate) async fn add_invocations(
        &mut self,
        block: u64,
        mut addresses: HashSet<String>,
    ) -> Result<(), sea_orm::DbErr> {
        assert_eq!(
            block,
            self.progress.last_finished_block + 1,
            "block number must be continuous"
        );
        let pending_addresses = self
            .pending_invocations
            .keys()
            .cloned()
            .collect::<HashSet<_>>();

        for pending_addr in pending_addresses {
            if !addresses.remove(&pending_addr) {
                let from_block = self
                    .pending_invocations
                    .remove(&pending_addr)
                    .expect("impossible: pending address not found");
                let from_block = from_block as i64;
                let block = block as i64;
                let invocation = entities::invocation::Model {
                    contract: pending_addr,
                    from_block,
                    to_block: block - 1,
                };
                self.invocations_to_insert.push(invocation.into());
                self.flush_invocations().await?;
            }
        }
        self.pending_invocations
            .extend(addresses.into_iter().map(|addr| (addr.clone(), block)));
        Ok(())
    }

    pub(crate) async fn add_creations(
        &mut self,
        block: u64,
        creations: Vec<(String, String, bool)>,
    ) -> Result<(), sea_orm::DbErr> {
        for (contract, tx, destruct) in creations {
            let block = block as i64;
            let creation = entities::creation::Model {
                contract,
                tx,
                block,
                destruct,
            };
            self.creations_to_insert.push(creation.into());
            self.flush_creations().await?;
        }
        Ok(())
    }

    pub(crate) fn add_failed_block(&mut self, block: u64) {
        self.progress.failed_blocks.push(block)
    }

    pub(crate) fn update_last_finished_block(&mut self, block: u64) {
        self.progress.last_finished_block = block;
    }

    pub(crate) async fn save_progress(&mut self) -> Result<(), sea_orm::DbErr> {
        info!(
            last_finished_block = self.progress.last_finished_block,
            "saving progress"
        );
        let progress = base_entities::metadata::ActiveModel::from((
            METADATA_KEY.to_string(),
            serde_json::to_value(&self.progress).unwrap(),
        ));
        let threshold = self.flush_threshold;
        self.flush_threshold = 0;
        self.flush_creations().await?;
        self.flush_invocations().await?;
        self.flush_threshold = threshold;
        base_entities::metadata::Entity::insert(progress)
            .on_conflict(
                sea_query::OnConflict::column(
                    base_entities::metadata::Column::Key,
                )
                .update_column(base_entities::metadata::Column::Value)
                .to_owned(),
            )
            .exec(self.db)
            .await?;
        Ok(())
    }

    // database operations

    async fn flush_invocations(&mut self) -> Result<(), sea_orm::DbErr> {
        if self.invocations_to_insert.len() > 0
            && self.invocations_to_insert.len() >= self.flush_threshold as usize
        {
            debug!(
                count = self.invocations_to_insert.len(),
                "flushing invocations to database"
            );
            let r = entities::invocation::Entity::insert_many(
                self.invocations_to_insert.clone(),
            )
            .on_conflict(
                sea_query::OnConflict::columns([
                    entities::invocation::Column::Contract,
                    entities::invocation::Column::FromBlock,
                ])
                .do_nothing()
                .to_owned(),
            )
            .exec(self.db)
            .await;
            if let Err(e) = r {
                if e != sea_orm::DbErr::RecordNotInserted {
                    return Err(e);
                }
            }
            self.invocations_to_insert.clear();
        }
        Ok(())
    }

    async fn flush_creations(&mut self) -> Result<(), sea_orm::DbErr> {
        if self.creations_to_insert.len() > 0
            && self.creations_to_insert.len() >= self.flush_threshold as usize
        {
            debug!(
                count = self.creations_to_insert.len(),
                "flushing creations to database"
            );
            let r = entities::creation::Entity::insert_many(
                self.creations_to_insert.clone(),
            )
            .on_conflict(
                sea_query::OnConflict::columns([
                    entities::creation::Column::Contract,
                    entities::creation::Column::Tx,
                ])
                .do_nothing()
                .to_owned(),
            )
            .exec(self.db)
            .await;
            if let Err(e) = r {
                if e != sea_orm::DbErr::RecordNotInserted {
                    return Err(e);
                }
            }
            self.creations_to_insert.clear();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use libsofl_knowledge_base::entities as base_entities;
    use sea_orm::{DatabaseBackend, MockDatabase, MockExecResult};

    #[tokio::test(flavor = "multi_thread")]
    #[should_panic(expected = "continuous")]
    async fn test_resume_progress() {
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([vec![base_entities::metadata::Model {
                key: super::METADATA_KEY.to_string(),
                value: serde_json::to_string(&super::Progress {
                    last_finished_block: 2,
                    failed_blocks: Vec::new(),
                })
                .unwrap()
                .to_string(),
            }]]);
        let connection = db.into_connection();
        let mut store = super::DataStore::new(&connection, 100).await.unwrap();
        store.add_invocations(1, HashSet::new()).await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_invocations_flush_threshold() {
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([vec![base_entities::metadata::Model {
                key: super::METADATA_KEY.to_string(),
                value: serde_json::to_string(&super::Progress {
                    last_finished_block: 0,
                    failed_blocks: Vec::new(),
                })
                .unwrap()
                .to_string(),
            }]])
            .append_exec_results([MockExecResult {
                last_insert_id: 1,
                rows_affected: 2,
            }]);
        let connection = db.into_connection();
        let mut store = super::DataStore::new(&connection, 2).await.unwrap();
        let mut addresses = HashSet::new();
        addresses.insert("0x1".to_string());
        store.add_invocations(1, addresses).await.unwrap(); // should be saved in pending addresses
        store.update_last_finished_block(1);
        let mut addresses = HashSet::new();
        addresses.insert("0x2".to_string());
        store.add_invocations(2, addresses).await.unwrap(); // should flush 0x1
        store.update_last_finished_block(2);
        let mut addresses = HashSet::new();
        addresses.insert("0x3".to_string());
        store.add_invocations(3, addresses).await.unwrap(); // should flush 0x2, and save both 0x1 and 0x2 in to database
        store.update_last_finished_block(3);

        let logs = connection.into_transaction_log();
        assert_eq!(logs.len(), 2); // two queries: check metadata, and one insert invocation.
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_creation_flush_threshold() {
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([vec![base_entities::metadata::Model {
                key: super::METADATA_KEY.to_string(),
                value: serde_json::to_string(&super::Progress {
                    last_finished_block: 0,
                    failed_blocks: Vec::new(),
                })
                .unwrap()
                .to_string(),
            }]])
            .append_exec_results([MockExecResult {
                last_insert_id: 1,
                rows_affected: 2,
            }]);
        let connection = db.into_connection();
        let mut store = super::DataStore::new(&connection, 2).await.unwrap();

        let creations = vec![("0x1".to_string(), "0x1".to_string(), false)];
        store.add_creations(1, creations).await.unwrap(); // should be flushed to cache
        store.update_last_finished_block(1);
        let creations = vec![("0x2".to_string(), "0x2".to_string(), false)];
        store.add_creations(2, creations).await.unwrap(); // should be flushed and save to database
        store.update_last_finished_block(2);

        let logs = connection.into_transaction_log();
        assert_eq!(logs.len(), 2); // two queries: check metadata, and one insert creation.
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_save_progress() {
        let db = MockDatabase::new(DatabaseBackend::Postgres)
            .append_query_results([vec![base_entities::metadata::Model {
                key: super::METADATA_KEY.to_string(),
                value: serde_json::to_string(&super::Progress {
                    last_finished_block: 0,
                    failed_blocks: Vec::new(),
                })
                .unwrap()
                .to_string(),
            }]])
            .append_exec_results([
                MockExecResult {
                    last_insert_id: 1,
                    rows_affected: 1,
                },
                MockExecResult {
                    last_insert_id: 2,
                    rows_affected: 1,
                },
            ])
            .append_exec_errors([]);
        let connection = db.into_connection();
        let mut store = super::DataStore::new(&connection, 2).await.unwrap();
        let creations = vec![("0x1".to_string(), "0x1".to_string(), false)];
        store.add_creations(1, creations).await.unwrap(); // should be flushed to cache
        store.add_failed_block(0);
        store.update_last_finished_block(1);
        store.save_progress().await.unwrap(); // save creation cache to database and save finished_block to metadata

        let logs = connection.into_transaction_log();
        assert_eq!(logs.len(), 3); // three queries: check metadata, insert creation, update metadata.
    }
}
