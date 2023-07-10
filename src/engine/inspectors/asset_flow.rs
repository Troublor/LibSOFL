use reth_primitives::{Address, Log};
use revm::{
    interpreter::{instruction_result::SuccessOrHalt, InstructionResult},
    Database, Inspector,
};
use revm_primitives::{ExecutionResult, Output, TransactTo, U256};
use serde::{Deserialize, Serialize};

use crate::utils::{
    abi::{ERC1155_ABI, ERC20_ABI, ERC721_ABI, ERC777_ABI, WETH_ABI},
    conversion::{Convert, ToEthers, ToPrimitive},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetKind {
    Ether,
    ERC20,
    ERC721,
    ERC777,
    ERC1155,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetTransfer {
    kind: AssetKind,

    from: Address,
    to: Address,

    /// The contract of the asset being transferred.
    /// For Ether, this is None.
    contract: Option<Address>,

    /// The amount being transferred.
    /// For Ether/ERC20/ERC777, this is the amount in wei.
    /// For ERC721, this is always 1.
    /// For ERC1155, this is the amount of the token being transferred.
    amount: U256,

    /// The asset being transferred.
    /// For Ether/ERC20/ERC777, this is None.
    /// For ERC72/ERC11551, this is the token ID.
    asset: Option<U256>,
}

impl AssetTransfer {
    pub fn new_ether(from: Address, to: Address, amount: U256) -> Self {
        Self {
            kind: AssetKind::Ether,
            from,
            to,
            contract: None,
            amount,
            asset: None,
        }
    }

    pub fn try_parse_log(log: &Log) -> Vec<Self> {
        let mut transfers = Vec::new();
        if let Some(transfer) = Self::try_parse_log_as_erc20(log) {
            transfers.push(transfer);
        } else if let Some(transfer) = Self::try_parse_log_as_erc721(log) {
            transfers.push(transfer);
        } else if let Some(transfer) = Self::try_parse_log_as_erc777(log) {
            transfers.push(transfer);
        } else if let Some(transfer) = Self::try_parse_log_as_erc1155(log) {
            transfers.extend(transfer);
        }
        transfers
    }

    pub fn try_parse_log_as_erc20(log: &Log) -> Option<Self> {
        let from;
        let to;
        let amount;
        if let Ok(mut parsed_log) = ERC20_ABI
            .event("Transfer")
            .expect("Transfer event not found in ERC20 ABI")
            .parse_log(ToEthers::cvt(log))
        {
            from = parsed_log.params.remove(0).value.into_address()?;
            to = parsed_log.params.remove(0).value.into_address()?;
            amount = parsed_log.params.remove(0).value.into_uint()?;
        } else if let Ok(mut parsed_log) = WETH_ABI
            .event("Deposit")
            .expect("Deposit event not found in WETH ABI")
            .parse_log(ToEthers::cvt(log))
        {
            from = Address::zero().into();
            to = parsed_log.params.remove(0).value.into_address()?;
            amount = parsed_log.params.remove(0).value.into_uint()?;
        } else if let Ok(mut parsed_log) = WETH_ABI
            .event("Withdrawal")
            .expect("Withdrawal event not found in WETH ABI")
            .parse_log(ToEthers::cvt(log))
        {
            from = parsed_log.params.remove(0).value.into_address()?;
            to = Address::zero().into();
            amount = parsed_log.params.remove(0).value.into_uint()?;
        } else {
            return None;
        }

        Some(Self {
            kind: AssetKind::ERC20,
            from: ToPrimitive::cvt(from),
            to: ToPrimitive::cvt(to),
            contract: Some(log.address),
            amount: ToPrimitive::cvt(amount),
            asset: None,
        })
    }

    pub fn try_parse_log_as_erc721(log: &Log) -> Option<Self> {
        let Ok(mut parsed_log) = ERC721_ABI
            .event("Transfer")
            .expect("Transfer event not found in ERC721 ABI")
            .parse_log(ToEthers::cvt(log))
        else {
            return None;
        };
        let from = parsed_log.params.remove(0).value.into_address()?;
        let to = parsed_log.params.remove(0).value.into_address()?;
        let asset = parsed_log.params.remove(0).value.into_uint()?;

        Some(Self {
            kind: AssetKind::ERC721,
            from: ToPrimitive::cvt(from),
            to: ToPrimitive::cvt(to),
            contract: Some(log.address),
            amount: ToPrimitive::cvt(1),
            asset: Some(ToPrimitive::cvt(asset)),
        })
    }

    pub fn try_parse_log_as_erc777(log: &Log) -> Option<Self> {
        let from;
        let to;
        let amount;
        if let Ok(mut parsed_log) = ERC777_ABI
            .event("Sent")
            .expect("Sent event not found in ERC777 ABI")
            .parse_log(ToEthers::cvt(log))
        {
            from = parsed_log.params.remove(1).value.into_address()?;
            to = parsed_log.params.remove(1).value.into_address()?;
            amount = parsed_log.params.remove(1).value.into_uint()?;
        } else if let Ok(mut parsed_log) = ERC777_ABI
            .event("Burned")
            .expect("Burned event not found in ERC777 ABI")
            .parse_log(ToEthers::cvt(log))
        {
            from = parsed_log.params.remove(1).value.into_address()?;
            to = Address::zero().into();
            amount = parsed_log.params.remove(1).value.into_uint()?;
        } else if let Ok(mut parsed_log) = ERC777_ABI
            .event("Minted")
            .expect("Minted event not found in ERC777 ABI")
            .parse_log(ToEthers::cvt(log))
        {
            from = Address::zero().into();
            to = parsed_log.params.remove(1).value.into_address()?;
            amount = parsed_log.params.remove(1).value.into_uint()?;
        } else {
            return None;
        }
        Some(Self {
            kind: AssetKind::ERC777,
            from: ToPrimitive::cvt(from),
            to: ToPrimitive::cvt(to),
            contract: Some(log.address),
            amount: ToPrimitive::cvt(amount),
            asset: None,
        })
    }

    pub fn try_parse_log_as_erc1155(log: &Log) -> Option<Vec<Self>> {
        let mut transfers = Vec::new();
        if let Ok(mut parsed_log) = ERC1155_ABI
            .event("TransferSingle")
            .expect("TransferSingle event not found in ERC1155 ABI")
            .parse_log(ToEthers::cvt(log))
        {
            let from = parsed_log.params.remove(1).value.into_address()?;
            let to = parsed_log.params.remove(1).value.into_address()?;
            let asset = parsed_log.params.remove(1).value.into_uint()?;
            let amount = parsed_log.params.remove(1).value.into_uint()?;
            let transfer = Self {
                kind: AssetKind::ERC1155,
                from: ToPrimitive::cvt(from),
                to: ToPrimitive::cvt(to),
                contract: Some(log.address),
                amount: ToPrimitive::cvt(amount),
                asset: Some(ToPrimitive::cvt(asset)),
            };
            transfers.push(transfer);
        } else if let Ok(mut parsed_log) = ERC1155_ABI
            .event("TransferBatch")
            .expect("TransferBatch event not found in ERC1155 ABI")
            .parse_log(ToEthers::cvt(log))
        {
            let from = parsed_log.params.remove(1).value.into_address()?;
            let to = parsed_log.params.remove(1).value.into_address()?;
            let assets = parsed_log.params.remove(1).value.into_array()?;
            let amounts = parsed_log.params.remove(1).value.into_array()?;
            for (asset, amount) in assets.into_iter().zip(amounts.into_iter()) {
                let asset = asset.into_uint()?;
                let amount = amount.into_uint()?;
                let transfer = Self {
                    kind: AssetKind::ERC1155,
                    from: ToPrimitive::cvt(from),
                    to: ToPrimitive::cvt(to),
                    contract: Some(log.address),
                    amount: ToPrimitive::cvt(amount),
                    asset: Some(ToPrimitive::cvt(asset)),
                };
                transfers.push(transfer);
            }
        }
        Some(transfers)
    }
}

/// Inspector that can be collects all money flow in each transaction.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AssetFlowInspector {
    /// The money flow in each transaction.
    /// Each transaction is represented as a list of transfers.
    /// tx_index => [transfer]
    pub transfers: Vec<Vec<AssetTransfer>>,

    /// The current transaction index.
    idx: usize,
    /// The stack of current call transfers, cached incase the call reverted.
    call_transfers_stack: Vec<Vec<AssetTransfer>>,
}

impl AssetFlowInspector {
    pub fn new() -> Self {
        Self::default()
    }
}

impl AssetFlowInspector {
    fn current_call_transfers(&mut self) -> &mut Vec<AssetTransfer> {
        self.call_transfers_stack
            .last_mut()
            .expect("bug: current call transfer vector not prepared")
    }

    fn start_call(&mut self) {
        self.call_transfers_stack.push(Vec::new());
    }

    fn finish_call(&mut self, success: bool) -> Vec<AssetTransfer> {
        let mut current = self
            .call_transfers_stack
            .pop()
            .expect("bug: call transfer stack is empty at the end of a call");
        if success && !self.call_transfers_stack.is_empty() {
            // aggregate the finished call's transfers to its parent call
            self.current_call_transfers().append(&mut current);
        }
        current
    }
}

impl<BS: Database> Inspector<BS> for AssetFlowInspector {
    fn create(
        &mut self,
        _data: &mut revm::EVMData<'_, BS>,
        _inputs: &mut revm::interpreter::CreateInputs,
    ) -> (
        InstructionResult,
        Option<revm_primitives::B160>,
        revm::interpreter::Gas,
        revm_primitives::Bytes,
    ) {
        self.start_call();
        (
            revm::interpreter::InstructionResult::Continue,
            None,
            revm::interpreter::Gas::new(0),
            revm_primitives::Bytes::new(),
        )
    }

    fn call(
        &mut self,
        _data: &mut revm::EVMData<'_, BS>,
        _inputs: &mut revm::interpreter::CallInputs,
        _is_static: bool,
    ) -> (
        revm::interpreter::InstructionResult,
        revm::interpreter::Gas,
        revm_primitives::Bytes,
    ) {
        self.start_call();
        // transfer ether to the contract
        if _inputs.transfer.value > U256::ZERO {
            let transfer = AssetTransfer::new_ether(
                _inputs.transfer.source,
                _inputs.transfer.target,
                _inputs.transfer.value,
            );
            self.current_call_transfers().push(transfer);
        }
        (
            revm::interpreter::InstructionResult::Continue,
            revm::interpreter::Gas::new(0),
            revm_primitives::Bytes::new(),
        )
    }

    fn log(
        &mut self,
        _evm_data: &mut revm::EVMData<'_, BS>,
        _address: &revm_primitives::B160,
        _topics: &[revm_primitives::B256],
        _data: &revm_primitives::Bytes,
    ) {
        let transfers = AssetTransfer::try_parse_log(&Log {
            address: *_address,
            topics: _topics.to_vec(),
            data: ToPrimitive::cvt(_data),
        });
        self.current_call_transfers().extend(transfers);
    }

    fn call_end(
        &mut self,
        _data: &mut revm::EVMData<'_, BS>,
        _inputs: &revm::interpreter::CallInputs,
        remaining_gas: revm::interpreter::Gas,
        ret: revm::interpreter::InstructionResult,
        out: revm_primitives::Bytes,
        _is_static: bool,
    ) -> (
        revm::interpreter::InstructionResult,
        revm::interpreter::Gas,
        revm_primitives::Bytes,
    ) {
        let outcome = SuccessOrHalt::from(ret);
        self.finish_call(outcome.is_success());
        (ret, remaining_gas, out)
    }

    fn create_end(
        &mut self,
        _data: &mut revm::EVMData<'_, BS>,
        _inputs: &revm::interpreter::CreateInputs,
        ret: InstructionResult,
        address: Option<revm_primitives::B160>,
        remaining_gas: revm::interpreter::Gas,
        out: revm_primitives::Bytes,
    ) -> (
        InstructionResult,
        Option<revm_primitives::B160>,
        revm::interpreter::Gas,
        revm_primitives::Bytes,
    ) {
        let outcome = SuccessOrHalt::from(ret);
        if outcome.is_success() && _inputs.value > U256::ZERO {
            let transfer = AssetTransfer::new_ether(
                _inputs.caller,
                address.expect(
                    "impossible: an success create call outputs no address",
                ),
                _inputs.value,
            );
            self.current_call_transfers().push(transfer);
        }
        self.finish_call(outcome.is_success());
        (ret, address, remaining_gas, out)
    }
}

impl<BS: Database> super::MultiTxInspector<BS> for AssetFlowInspector {
    fn transaction(
        &mut self,
        _tx: &revm_primitives::TxEnv,
        _state: &BS,
    ) -> bool {
        self.start_call();

        true
    }

    fn transaction_end(
        &mut self,
        tx: &revm_primitives::TxEnv,
        _state: &BS,
        _result: &ExecutionResult,
    ) {
        if let ExecutionResult::Success { output, .. } = _result {
            if tx.value > U256::ZERO
                && matches!(tx.transact_to, TransactTo::Create(_))
            {
                // transfer to newly created contract
                let to =  match output {
                        Output::Create(_, Some(address)) => *address,
                        _ => panic!("Invalid transaction: a success contract creation transaction does not output an address"),
                    };
                let transfer =
                    AssetTransfer::new_ether(tx.caller, to, tx.value);
                self.current_call_transfers().push(transfer);
            }
        }
        let transfers = self.finish_call(_result.is_success());
        self.transfers.push(transfers);
    }
}

#[cfg(test)]
mod tests_with_dep {
    use reth_primitives::TxHash;

    use crate::{
        engine::{
            inspectors::asset_flow::AssetKind,
            state::{env::TransitionSpec, BcState, BcStateBuilder},
        },
        utils::{
            conversion::{Convert, ToPrimitive},
            testing::get_testing_bc_provider,
        },
    };

    use super::AssetFlowInspector;

    #[test]
    fn test_transfers_in_plain_transaction() {
        let provider = get_testing_bc_provider();
        let state = BcStateBuilder::fork_at(&provider, 16000000).unwrap();
        let tx: TxHash = ToPrimitive::cvt("0x6b3fa0f8c6a87b9c8951e96dd44c5d4635f1bbf056040d9a626f344496b6ce54");
        let transition_spec =
            TransitionSpec::from_tx_hash(&provider, tx).unwrap();
        let mut insp = AssetFlowInspector::new();
        let _ = BcState::transit(state, transition_spec, &mut insp).unwrap();
        assert_eq!(insp.transfers.len(), 1);
        let transfers = &insp.transfers[0];
        assert_eq!(transfers.len(), 1);
        let transfer = &transfers[0];
        assert_eq!(transfer.kind, AssetKind::Ether);
        assert_eq!(
            transfer.from,
            ToPrimitive::cvt("0x78eC5C6265B45B9c98CF682665A00A3E8f085fFE")
        );
        assert_eq!(
            transfer.to,
            ToPrimitive::cvt("0x4E41e19f939a0040330F7Cd3CFfde8cA96700d9b")
        );
        assert_eq!(
            transfer.amount,
            ToPrimitive::cvt(ethers::utils::parse_ether("0.002312").unwrap())
        );
    }

    #[test]
    fn test_no_transfer() {
        let provider = get_testing_bc_provider();
        let state = BcStateBuilder::fork_at(&provider, 16000000).unwrap();
        let tx: TxHash = ToPrimitive::cvt("0xd801d27211b0dfcfdff7e370069268e6fb3ef08ea25148c1065718482c4eab32");
        let spec = TransitionSpec::from_tx_hash(&provider, tx).unwrap();
        let mut insp = AssetFlowInspector::new();
        let _ = BcState::transit(state, spec, &mut insp).unwrap();
        assert_eq!(insp.transfers.len(), 1);
        let transfers = &insp.transfers[0];
        assert_eq!(transfers.len(), 0);
    }

    #[test]
    fn test_token_transfer() {
        let provider = get_testing_bc_provider();
        let state = BcStateBuilder::fork_at(&provider, 16000000).unwrap();
        let tx: TxHash = ToPrimitive::cvt("0x90c93f15f470569d0339a28a6d0d0af7eeaeb6b40e6e53eb56016158119906dc");
        let spec = TransitionSpec::from_tx_hash(&provider, tx).unwrap();
        let mut insp = AssetFlowInspector::new();
        let _ = BcState::transit(state, spec, &mut insp).unwrap();
        assert_eq!(insp.transfers.len(), 1);
        let transfers = &insp.transfers[0];
        assert_eq!(transfers.len(), 6);
    }

    #[test]
    fn test_snood_spend_allowance_attack() {
        // attack tx: 0x9a6227ef97d7ce75732645bd604ef128bb5dfbc1bfbe0966ad1cd2870d45a20e
        let provider = get_testing_bc_provider();
        let state = BcStateBuilder::fork_at(&provider, 14983664).unwrap();
        let tx: TxHash = ToPrimitive::cvt("0x9a6227ef97d7ce75732645bd604ef128bb5dfbc1bfbe0966ad1cd2870d45a20e");
        let spec = TransitionSpec::from_tx_hash(&provider, tx).unwrap();
        let mut insp = AssetFlowInspector::new();
        let _ = BcState::transit(state, spec, &mut insp).unwrap();
        assert_eq!(insp.transfers.len(), 1);
        let transfers = &insp.transfers[0];
        assert_eq!(transfers.len(), 11);
    }
}
