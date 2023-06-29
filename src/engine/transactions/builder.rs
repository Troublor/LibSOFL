use reth_primitives::Address;
use revm_primitives::{BlockEnv, CfgEnv, U256};

use crate::utils::conversion::{Convert, ToElementary};

use super::Tx;

#[derive(Debug, Clone, Default)]
pub struct TxBuilder {
    chain_id: Option<u64>,
    from: Option<Address>,
    to: Option<Option<Address>>,
    nonce: Option<u64>,
    value: Option<u128>,
    input: Option<reth_primitives::Bytes>,
    gas_limit: Option<u64>,
    gas_price: Option<u128>, // also used as max_fee_per_gas in EIP1559 transactions
    priority_fee: Option<u128>,
    access_list: Option<reth_primitives::AccessList>,
}

impl TxBuilder {
    pub fn new() -> Self {
        Self::default()
    }
}

impl TxBuilder {
    pub fn build(self) -> Tx {
        let chain_id = self.chain_id.unwrap_or(1);
        let from = self.from.unwrap_or(Address::zero());
        let to = self
            .to
            .unwrap_or(None)
            .map(reth_primitives::TransactionKind::Call)
            .unwrap_or(reth_primitives::TransactionKind::Create);
        let nonce = self.nonce.unwrap_or(0);
        let value = self.value.unwrap_or(0);
        let input = self.input.unwrap_or_default();
        let gas_limit = self.gas_limit.unwrap_or(0);
        let gas_price = self.gas_price.unwrap_or(0);
        if let Some(priority_fee) = self.priority_fee {
            // EIP1559 transaction
            let access_list = self.access_list.unwrap_or_default();
            (
                from,
                reth_primitives::Transaction::Eip1559(
                    reth_primitives::TxEip1559 {
                        chain_id,
                        nonce,
                        gas_limit,
                        max_fee_per_gas: gas_price,
                        max_priority_fee_per_gas: priority_fee,
                        to,
                        value,
                        access_list,
                        input,
                    },
                ),
            )
                .into()
        } else if let Some(access_list) = self.access_list {
            (
                from,
                reth_primitives::Transaction::Eip2930(
                    reth_primitives::TxEip2930 {
                        chain_id,
                        nonce,
                        gas_price,
                        gas_limit,
                        to,
                        value,
                        access_list,
                        input,
                    },
                ),
            )
                .into()
        } else {
            (
                from,
                reth_primitives::Transaction::Legacy(
                    reth_primitives::TxLegacy {
                        chain_id: Some(chain_id),
                        nonce,
                        gas_price,
                        gas_limit,
                        to,
                        value,
                        input,
                    },
                ),
            )
                .into()
        }
    }

    pub fn fit_evm_cfg(mut self, cfg: CfgEnv) -> Self {
        self.chain_id = Some(ToElementary::cvt(cfg.chain_id));
        self
    }

    pub fn fit_block_env(mut self, block: BlockEnv) -> Self {
        // ensure gas_price >= base_fee + priority_fee
        let base_fee: u128 = ToElementary::cvt(block.basefee);
        let priority_fee = self.priority_fee.unwrap_or(0);
        let gas_price = self.gas_price.unwrap_or(0);
        if base_fee + priority_fee > gas_price {
            self.gas_price = Some(base_fee + priority_fee);
        }

        // ensure gas_limit <= block.gas_limit
        let gas_limit = self.gas_limit.unwrap_or(0);
        let block_gas_limit: u64 = ToElementary::cvt(block.gas_limit);
        if gas_limit > block_gas_limit {
            self.gas_limit = Some(block_gas_limit);
        }

        self
    }

    pub fn set_chain_id(mut self, chain_id: u64) -> Self {
        self.chain_id = Some(chain_id);
        self
    }

    pub fn set_from(mut self, from: Address) -> Self {
        self.from = Some(from);
        self
    }

    pub fn set_to(mut self, to: Address) -> Self {
        self.to = Some(Some(to));
        self
    }

    pub fn unset_to(mut self) -> Self {
        self.to = Some(None);
        self
    }

    pub fn set_nonce(mut self, nonce: u64) -> Self {
        self.nonce = Some(nonce);
        self
    }

    /// Fit the nonce to the from account's nonce if from is already set.
    /// Otherwise, do nothing.
    pub fn fit_account_nonce(
        mut self,
        p: impl reth_provider::AccountReader,
    ) -> Self {
        if let Some(from) = self.from {
            let nonce = p
                .basic_account(from)
                .expect("AccountReader is not usable")
                .map(|a| a.nonce)
                .unwrap_or(0);
            self.nonce = Some(nonce);
        }
        self
    }

    pub fn set_value(mut self, value: U256) -> Self {
        self.value = Some(ToElementary::cvt(value));
        self
    }

    pub fn set_input<I: Into<reth_primitives::Bytes>>(
        mut self,
        input: I,
    ) -> Self {
        self.input = Some(input.into());
        self
    }

    pub fn set_input_with_low_level_call(
        mut self,
        sighash: [u8; 4],
        args: &[u8],
    ) -> Self {
        self.input = Some([sighash.as_slice(), args].concat().into());
        self
    }

    pub fn set_input_with_high_level_call(
        mut self,
        func: &ethers::abi::Function,
        args: &[ethers::abi::Token],
    ) -> Self {
        let input = func.encode_input(args).expect("invalid args");
        self.input = Some(input.into());
        self
    }

    pub fn set_gas_limit(mut self, gas_limit: u64) -> Self {
        self.gas_limit = Some(gas_limit);
        self
    }

    pub fn set_gas_price(mut self, gas_price: u128) -> Self {
        self.gas_price = Some(gas_price);
        self
    }

    pub fn set_priority_fee(mut self, priority_fee: u128) -> Self {
        self.priority_fee = Some(priority_fee);
        self
    }

    pub fn set_access_list(
        mut self,
        access_list: reth_primitives::AccessList,
    ) -> Self {
        self.access_list = Some(access_list);
        self
    }
}
