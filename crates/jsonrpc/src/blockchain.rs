use alloy_rpc_types::{Transaction, TransactionReceipt};
use libsofl_core::{
    blockchain::{
        transaction::{Log, Tx},
        tx_position::TxPosition,
    },
    conversion::ConvertTo,
    engine::types::{
        Address, BlockHashOrNumber, Bytes, CreateScheme, TransactTo, TxEnv, TxHash, U256,
    },
    error::SoflError,
};

#[derive(Debug, Clone)]
pub struct JsonRpcTx {
    pub(crate) tx: Transaction,
    pub(crate) receipt: Option<TransactionReceipt>,
}

impl Tx for JsonRpcTx {
    #[doc = " Returns the hash of the transaction."]
    fn hash(&self) -> TxHash {
        self.tx.hash
    }

    #[doc = " Returns the sender of the transaction."]
    fn sender(&self) -> Address {
        self.tx.from
    }

    #[doc = " Returns the value of the transaction."]
    fn value(&self) -> U256 {
        self.tx.value
    }

    #[doc = " Returns the input data of the transaction."]
    fn input(&self) -> Bytes {
        self.tx.input.clone()
    }

    #[doc = " Fill the revm transaction environment."]
    fn fill_tx_env(&self, env: &mut TxEnv) -> Result<(), SoflError> {
        env.caller = self.sender();
        env.gas_limit = self.tx.gas.cvt();
        env.gas_price = self.tx.gas_price.map(|p| p.cvt()).unwrap_or_default();
        env.transact_to = match self.tx.to {
            Some(to) => TransactTo::Call(to),
            None => TransactTo::Create(CreateScheme::Create),
        };
        env.value = self.value();
        env.data = self.input();
        env.nonce = Some(self.tx.nonce.cvt());
        env.chain_id = self.tx.chain_id.map(|c| c.cvt());
        env.access_list = self
            .tx
            .access_list
            .clone()
            .map(|l| {
                l.into_iter()
                    .map(|a| {
                        (
                            a.address,
                            a.storage_keys.into_iter().map(|k| k.cvt()).collect(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        env.gas_priority_fee = self.tx.max_priority_fee_per_gas.map(|p| p.cvt());
        env.blob_hashes = self.tx.blob_versioned_hashes.clone();
        env.max_fee_per_blob_gas = self.tx.max_fee_per_blob_gas.map(|p| p.cvt());
        Ok(())
    }

    #[doc = " Only available after tx execution"]
    #[doc = " Returns the position of the transaction in the blockchain."]
    #[doc = " None if the transaction is not in the blockchain."]
    fn position(&self) -> Option<TxPosition> {
        self.tx
            .block_number
            .zip(self.tx.transaction_index)
            .map(|(block_number, index)| TxPosition {
                block: BlockHashOrNumber::Number(block_number.cvt()),
                index: index.cvt(),
            })
    }

    #[doc = " Returns the output data of the transaction."]
    #[doc = " None if the transaction is not executed."]
    fn output(&self) -> Option<Bytes> {
        None
    }

    #[doc = " Returns whether the transaction succeeds."]
    #[doc = " None if the transaction is not executed."]
    fn success(&self) -> Option<bool> {
        self.receipt
            .clone()?
            .status_code
            .map(|code| !code.is_zero())
    }

    #[doc = " Returns the gas used by the transaction."]
    #[doc = " None if the transaction is not executed."]
    fn logs(&self) -> Option<Vec<Log>> {
        if self.success().is_none() {
            None
        } else {
            Some(
                self.receipt
                    .clone()?
                    .logs
                    .iter()
                    .map(|l| Log {
                        address: l.address,
                        topics: l.topics.clone(),
                        data: l.data.clone(),
                    })
                    .collect(),
            )
        }
    }

    #[doc = " Returns the recipient of the transaction."]
    fn to(&self) -> Option<Address> {
        self.tx.to
    }
}
