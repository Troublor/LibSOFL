use libsofl_core::{
    blockchain::{
        transaction::{Log, Tx},
        tx_position::TxPosition,
    },
    engine::types::{Address, Bytes, TxEnv, TxHash, U256},
    error::SoflError,
};
use reth_primitives::revm::env::fill_tx_env;
use reth_primitives::{TransactionMeta, TransactionSigned};
use reth_provider::{ReceiptProvider, TransactionsProvider};

use crate::conversion::ConvertTo;

use super::provider::RethBlockchainProvider;

/// A wrapper of reth transaction.
pub struct RethTx {
    pub tx: TransactionSigned,
    pub(crate) hash: TxHash,
    pub(crate) sender: Address,

    // only availabe after tx execution
    pub(crate) meta: Option<TransactionMeta>,
    pub(crate) success: Option<bool>,
    pub(crate) output: Option<Bytes>, // TODO: how to get evm output?
    pub(crate) logs: Option<Vec<Log>>,
}

impl From<TransactionSigned> for RethTx {
    fn from(tx: TransactionSigned) -> Self {
        let hash = tx.hash();
        let sender = tx
            .recover_signer_unchecked()
            .expect(format!("invalid signature for tx {}", hash).as_str())
            .cvt();
        Self {
            tx,
            sender,
            hash,
            meta: None,
            success: None,
            output: None,
            logs: None,
        }
    }
}

impl RethTx {
    pub fn from_hash(
        bp: &RethBlockchainProvider,
        hash: TxHash,
    ) -> Result<Self, SoflError> {
        let (tx, meta) = bp
            .transaction_by_hash_with_meta(hash)
            .map_err(|e| {
                SoflError::Provider(format!(
                    "failed to get transaction by hash: {}",
                    e
                ))
            })?
            .ok_or(SoflError::NotFound(format!("transaction {}", hash)))?;
        let mut tx: RethTx = tx.into();
        tx.meta = Some(meta);

        // fill receipt if available
        let receipt = bp.receipt_by_hash(hash).map_err(|e| {
            SoflError::Provider(format!("failed to get receipt by hash: {}", e))
        })?;
        if let Some(receipt) = receipt {
            let success = receipt.success;
            let logs = receipt.logs.into_iter().map(|log| log.cvt()).collect();
            tx.success = Some(success);
            tx.logs = Some(logs);
        }
        Ok(tx)
    }
}

impl Tx for RethTx {
    #[doc = " Returns the position of the transaction in the blockchain."]
    #[doc = " None if the transaction is not in the blockchain."]
    fn position(&self) -> Option<TxPosition> {
        if let Some(meta) = &self.meta {
            Some((meta.block_number, meta.index).into())
        } else {
            None
        }
    }

    #[doc = " Returns the sender of the transaction."]
    fn sender(&self) -> Address {
        self.sender
    }

    #[doc = " Returns the value of the transaction."]
    fn value(&self) -> U256 {
        self.tx.value().into()
    }

    #[doc = " Returns the input data of the transaction."]
    fn input(&self) -> Bytes {
        self.tx.input().cvt()
    }

    #[doc = " Fill the revm transaction environment."]
    fn fill_tx_env(&self, env: &mut TxEnv) -> Result<(), SoflError> {
        let mut reth_env: reth_primitives::revm_primitives::TxEnv =
            env.clone().cvt();
        fill_tx_env(
            &mut reth_env,
            Box::new(self.tx.transaction.clone()),
            self.sender(),
        );
        env.caller = reth_env.caller.cvt();
        env.gas_limit = reth_env.gas_limit;
        env.gas_price = reth_env.gas_price.cvt();
        env.transact_to = match reth_env.transact_to {
            reth_primitives::revm_primitives::TransactTo::Create(scheme) => {
                let scheme = match scheme {
                    reth_primitives::revm_primitives::CreateScheme::Create => {
                        libsofl_core::engine::revm::primitives::CreateScheme::Create
                    }
                    reth_primitives::revm_primitives::CreateScheme::Create2 {
                        salt,
                    } => libsofl_core::engine::revm::primitives::CreateScheme::Create2 {
                        salt: salt.cvt(),
                    },
                };
                libsofl_core::engine::revm::primitives::TransactTo::Create(
                    scheme,
                )
            }
            reth_primitives::revm_primitives::TransactTo::Call(addr) => {
                libsofl_core::engine::revm::primitives::TransactTo::Call(
                    addr.cvt(),
                )
            }
        };
        env.value = reth_env.value.cvt();
        env.data = reth_env.data.cvt();
        env.nonce = reth_env.nonce;
        env.chain_id = reth_env.chain_id;
        env.access_list = reth_env
            .access_list
            .into_iter()
            .map(|al| {
                (
                    al.0.cvt(),
                    al.1.into_iter().map(|slot| slot.cvt()).collect(),
                )
            })
            .collect();
        env.gas_priority_fee = reth_env.gas_priority_fee.map(ConvertTo::cvt);
        env.blob_hashes = reth_env
            .blob_hashes
            .into_iter()
            .map(ConvertTo::cvt)
            .collect();
        env.max_fee_per_blob_gas =
            reth_env.max_fee_per_blob_gas.map(ConvertTo::cvt);
        Ok(())
    }

    #[doc = " Returns the hash of the transaction."]
    fn hash(&self) -> TxHash {
        self.hash
    }

    #[doc = " Returns the output data of the transaction."]
    #[doc = " None if the transaction is not executed."]
    fn output(&self) -> Option<Bytes> {
        self.output.clone()
    }

    #[doc = " Returns whether the transaction succeeds."]
    #[doc = " None if the transaction is not executed."]
    fn success(&self) -> Option<bool> {
        self.success
    }

    #[doc = " Returns the gas used by the transaction."]
    #[doc = " None if the transaction is not executed."]
    fn logs(&self) -> Option<Vec<Log>> {
        self.logs.clone()
    }

    #[doc = " Returns the recipient of the transaction."]
    fn to(&self) -> Option<Address> {
        self.tx.to()
    }
}
