use reth_primitives::{Address, Log};
use revm::{
    interpreter::{instruction_result::SuccessOrHalt, InstructionResult},
    Database, Inspector,
};
use revm_primitives::{ExecutionResult, Output, TransactTo, B256, U256};
use serde::{Deserialize, Serialize};

use crate::utils::{
    abi::{ERC1155_ABI, ERC20_ABI, ERC721_ABI, ERC777_ABI, WETH_ABI},
    conversion::{Convert, ToEthers, ToPrimitive},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Asset {
    /// Ether(amount)
    Ether(U256),
    /// ERC20(contract, amount)
    ERC20(Address, U256),
    /// ERC721(contract, NFT_id)
    ERC721(Address, B256),
    /// ERC777(contract, amount)
    ERC777(Address, U256),
    /// ERC1155(contract, token_id, amount)
    ERC1155(Address, B256, U256),
}

impl PartialOrd for Asset {
    /// return none if assets are not comparable
    /// return Some(Ordering) if assets are comparable
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (Asset::Ether(a0), Asset::Ether(a1)) => Some(a0.cmp(a1)),
            (Asset::ERC20(c0, a0), Asset::ERC20(c1, a1)) => {
                if c0 == c1 {
                    Some(a0.cmp(a1))
                } else {
                    None
                }
            }
            (Asset::ERC721(c0, a0), Asset::ERC721(c1, a1)) => {
                if c0 == c1 || a0 == a1 {
                    Some(std::cmp::Ordering::Equal)
                } else {
                    None
                }
            }
            (Asset::ERC777(c0, a0), Asset::ERC777(c1, a1)) => {
                if c0 == c1 {
                    Some(a0.cmp(a1))
                } else {
                    None
                }
            }
            (Asset::ERC1155(c0, i0, a0), Asset::ERC1155(c1, i1, a1)) => {
                if c0 == c1 && i0 == i1 {
                    Some(a0.cmp(a1))
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

impl std::ops::Add for Asset {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Asset::Ether(a0), Asset::Ether(a1)) => {
                Asset::Ether(a0.checked_add(a1).expect("overflow"))
            }
            (Asset::ERC20(c0, a0), Asset::ERC20(c1, a1)) => {
                if c0 == c1 {
                    Asset::ERC20(c0, a0.checked_add(a1).expect("overflow"))
                } else {
                    panic!("cannot add different ERC20 assets")
                }
            }
            (Asset::ERC721(_, _), Asset::ERC721(_, _)) => {
                panic!("cannot add different ERC721 assets")
            }
            (Asset::ERC777(c0, a0), Asset::ERC777(c1, a1)) => {
                if c0 == c1 {
                    Asset::ERC777(c0, a0.checked_add(a1).expect("overflow"))
                } else {
                    panic!("cannot add different ERC777 assets")
                }
            }
            (Asset::ERC1155(c0, i0, a0), Asset::ERC1155(c1, i1, a1)) => {
                if c0 == c1 && i0 == i1 {
                    Asset::ERC1155(
                        c0,
                        i0,
                        a0.checked_add(a1).expect("overflow"),
                    )
                } else {
                    panic!("cannot add different ERC1155 assets")
                }
            }
            _ => panic!("cannot add different assets"),
        }
    }
}

impl std::ops::Sub for Asset {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Asset::Ether(a0), Asset::Ether(a1)) => {
                Asset::Ether(a0.checked_sub(a1).expect("underflow"))
            }
            (Asset::ERC20(c0, a0), Asset::ERC20(c1, a1)) => {
                if c0 == c1 {
                    Asset::ERC20(c0, a0.checked_sub(a1).expect("underflow"))
                } else {
                    panic!("cannot sub different ERC20 assets")
                }
            }
            (Asset::ERC721(_, _), Asset::ERC721(_, _)) => {
                panic!("cannot sub different ERC721 assets")
            }
            (Asset::ERC777(c0, a0), Asset::ERC777(c1, a1)) => {
                if c0 == c1 {
                    Asset::ERC777(c0, a0.checked_sub(a1).expect("underflow"))
                } else {
                    panic!("cannot sub different ERC777 assets")
                }
            }
            (Asset::ERC1155(c0, i0, a0), Asset::ERC1155(c1, i1, a1)) => {
                if c0 == c1 && i0 == i1 {
                    Asset::ERC1155(
                        c0,
                        i0,
                        a0.checked_sub(a1).expect("underflow"),
                    )
                } else {
                    panic!("cannot sub different ERC1155 assets")
                }
            }
            _ => panic!("cannot sub different assets"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetChange {
    Gain(Asset),
    Loss(Asset),
    Zero,
}

impl PartialOrd for AssetChange {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match (self, other) {
            (AssetChange::Gain(a0), AssetChange::Gain(a1)) => {
                a0.partial_cmp(a1)
            }
            (AssetChange::Gain(a0), AssetChange::Loss(a1)) => {
                a0.partial_cmp(a1).map(|_| std::cmp::Ordering::Greater)
            }
            (AssetChange::Gain(_), AssetChange::Zero) => {
                Some(std::cmp::Ordering::Greater)
            }
            (AssetChange::Loss(a0), AssetChange::Loss(a1)) => {
                a0.partial_cmp(a1).map(|o| o.reverse())
            }
            (AssetChange::Loss(_), AssetChange::Zero) => {
                Some(std::cmp::Ordering::Less)
            }
            (AssetChange::Zero, AssetChange::Zero) => {
                Some(std::cmp::Ordering::Equal)
            }
            _ => other.partial_cmp(self).map(|o| o.reverse()),
        }
    }
}

impl std::ops::Add for AssetChange {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (AssetChange::Gain(a0), AssetChange::Gain(a1)) => {
                AssetChange::Gain(a0 + a1)
            }
            (AssetChange::Gain(a0), AssetChange::Loss(a1)) => {
                match a0.partial_cmp(&a1) {
                    Some(std::cmp::Ordering::Greater) => {
                        AssetChange::Gain(a0 - a1)
                    }
                    Some(std::cmp::Ordering::Equal) => AssetChange::Zero,
                    Some(std::cmp::Ordering::Less) => {
                        AssetChange::Loss(a1 - a0)
                    }
                    None => panic!("cannot add different assets"),
                }
            }
            (AssetChange::Gain(a0), AssetChange::Zero) => AssetChange::Gain(a0),
            (AssetChange::Loss(a0), AssetChange::Loss(a1)) => {
                AssetChange::Loss(a0 + a1)
            }
            (AssetChange::Loss(a0), AssetChange::Zero) => AssetChange::Loss(a0),
            (AssetChange::Zero, AssetChange::Zero) => AssetChange::Zero,
            _ => rhs + self,
        }
    }
}

impl std::ops::Sub for AssetChange {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        self + (-rhs)
    }
}

impl std::ops::Neg for AssetChange {
    type Output = Self;
    fn neg(self) -> Self::Output {
        match self {
            AssetChange::Gain(a) => AssetChange::Loss(a),
            AssetChange::Loss(a) => AssetChange::Gain(a),
            AssetChange::Zero => AssetChange::Zero,
        }
    }
}

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    Default,
    derive_more::AsRef,
    derive_more::AsMut,
    derive_more::Deref,
    derive_more::DerefMut,
    derive_more::From,
)]
pub struct AssetsChange {
    #[as_ref]
    #[as_mut]
    #[deref]
    #[deref_mut]
    changes: Vec<AssetChange>,
}

impl From<AssetChange> for AssetsChange {
    fn from(change: AssetChange) -> Self {
        Self {
            changes: vec![change],
        }
    }
}

impl std::ops::Add<AssetChange> for AssetsChange {
    type Output = Self;
    fn add(self, rhs: AssetChange) -> Self::Output {
        let changes = self
            .changes
            .into_iter()
            .map(|c| {
                if c.partial_cmp(&rhs).is_some() {
                    c + rhs
                } else {
                    c
                }
            })
            .filter(|c| *c != AssetChange::Zero)
            .collect();
        Self { changes }
    }
}

impl std::ops::AddAssign<AssetChange> for AssetsChange {
    fn add_assign(&mut self, rhs: AssetChange) {
        let mut zero_indices = Vec::new();
        let mut merged = false;
        for i in 0..self.changes.len() {
            let change =
                self.changes.get(i).expect("impossible: index out of range");
            if change.partial_cmp(&rhs).is_some() {
                let new_change = *change + rhs;
                merged = true;
                if new_change == AssetChange::Zero {
                    zero_indices.push(i);
                } else {
                    self.changes[i] = new_change;
                }
            }
        }
        if !merged {
            self.changes.push(rhs);
        }
        for i in zero_indices.into_iter().rev() {
            self.changes.swap_remove(i);
        }
    }
}

impl std::ops::Add for AssetsChange {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        let mut changes = Self::default();
        for change in rhs.changes {
            changes += change;
        }
        changes
    }
}

impl std::ops::AddAssign for AssetsChange {
    fn add_assign(&mut self, rhs: Self) {
        for change in rhs.changes {
            *self += change;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetTransfer {
    sender: Address,
    receiver: Address,
    asset: Asset,
}

impl AssetTransfer {
    pub fn new_ether(from: Address, to: Address, amount: U256) -> Self {
        let asset = Asset::Ether(amount);
        Self {
            sender: from,
            receiver: to,
            asset,
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
            sender: ToPrimitive::cvt(from),
            receiver: ToPrimitive::cvt(to),
            asset: Asset::ERC20(log.address, ToPrimitive::cvt(amount)),
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
            sender: ToPrimitive::cvt(from),
            receiver: ToPrimitive::cvt(to),
            asset: Asset::ERC721(log.address, ToPrimitive::cvt(asset)),
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
            sender: ToPrimitive::cvt(from),
            receiver: ToPrimitive::cvt(to),
            asset: Asset::ERC777(log.address, ToPrimitive::cvt(amount)),
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
                sender: ToPrimitive::cvt(from),
                receiver: ToPrimitive::cvt(to),
                asset: Asset::ERC1155(
                    log.address,
                    ToPrimitive::cvt(amount),
                    ToPrimitive::cvt(asset),
                ),
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
                    sender: ToPrimitive::cvt(from),
                    receiver: ToPrimitive::cvt(to),
                    asset: Asset::ERC1155(
                        log.address,
                        ToPrimitive::cvt(asset),
                        ToPrimitive::cvt(amount),
                    ),
                };
                transfers.push(transfer);
            }
        }
        Some(transfers)
    }
}

impl AssetTransfer {
    pub fn get_sender_change(&self) -> AssetChange {
        AssetChange::Loss(self.asset)
    }

    pub fn get_receiver_change(&self) -> AssetChange {
        AssetChange::Gain(self.asset)
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

    mod asset {
        use crate::{
            engine::inspectors::asset_flow::{
                Asset, AssetChange, AssetsChange,
            },
            utils::{
                addresses,
                conversion::{Convert, ToPrimitive},
            },
        };

        #[test]
        fn test_asset_partial_compare() {
            // Comparable ERC20 assets
            let asset0 = Asset::ERC20(*addresses::DAI, ToPrimitive::cvt(12));
            let asset1 = Asset::ERC20(*addresses::DAI, ToPrimitive::cvt(123));
            assert!(asset0 < asset1);

            // Two different ERC20 tokens are not comparable
            let asset0 = Asset::ERC20(*addresses::DAI, ToPrimitive::cvt(12));
            let asset1 = Asset::ERC20(*addresses::USDC, ToPrimitive::cvt(123));
            assert_eq!(asset0.partial_cmp(&asset1), None);

            // Two different kind of assets are not comparable
            let asset0 = Asset::ERC20(*addresses::DAI, ToPrimitive::cvt(123));
            let asset1 = Asset::Ether(ToPrimitive::cvt(123));
            assert_eq!(asset0.partial_cmp(&asset1), None);
        }

        #[test]
        fn test_asset_change_merge() {
            let asset0 = Asset::ERC20(*addresses::DAI, ToPrimitive::cvt(12));
            let asset1 = Asset::ERC20(*addresses::DAI, ToPrimitive::cvt(123));
            let mut assets = AssetsChange::from(AssetChange::Gain(asset0));
            assets += AssetChange::Gain(asset1);
            assert_eq!(assets.len(), 1);
            assert_eq!(
                assets[0],
                AssetChange::Gain(Asset::ERC20(
                    *addresses::DAI,
                    ToPrimitive::cvt(135)
                ))
            );

            let asset0 =
                Asset::ERC777(*addresses::WBTC, ToPrimitive::cvt(1234));
            let asset1 = Asset::Ether(ToPrimitive::cvt(12345));
            let asset2 = Asset::ERC777(*addresses::WBTC, ToPrimitive::cvt(123));
            let mut assets = AssetsChange::from(AssetChange::Gain(asset0));
            assets += AssetChange::Gain(asset1);
            assert_eq!(assets.len(), 2);

            assets += AssetChange::Loss(asset2);
            assert_eq!(assets.len(), 2);
            assert_eq!(
                assets[0],
                AssetChange::Gain(Asset::ERC777(
                    *addresses::WBTC,
                    ToPrimitive::cvt(1111)
                ))
            );
        }
    }

    mod asset_flow_inspector {
        use reth_primitives::TxHash;
        use revm_primitives::U256;

        use crate::{
            engine::{
                inspectors::asset_flow::Asset,
                state::{env::TransitionSpec, BcState, BcStateBuilder},
            },
            utils::{
                conversion::{Convert, ToPrimitive},
                testing::get_testing_bc_provider,
            },
        };

        use super::super::AssetFlowInspector;

        #[test]
        fn test_transfers_in_plain_transaction() {
            let provider = get_testing_bc_provider();
            let state = BcStateBuilder::fork_at(&provider, 16000000).unwrap();
            let tx: TxHash = ToPrimitive::cvt("0x6b3fa0f8c6a87b9c8951e96dd44c5d4635f1bbf056040d9a626f344496b6ce54");
            let transition_spec =
                TransitionSpec::from_tx_hash(&provider, tx).unwrap();
            let mut insp = AssetFlowInspector::new();
            let _ =
                BcState::transit(state, transition_spec, &mut insp).unwrap();
            assert_eq!(insp.transfers.len(), 1);
            let transfers = &insp.transfers[0];
            assert_eq!(transfers.len(), 1);
            let transfer = &transfers[0];
            assert!(matches!(transfer.asset, Asset::Ether(_)));
            assert_eq!(
                transfer.sender,
                ToPrimitive::cvt("0x78eC5C6265B45B9c98CF682665A00A3E8f085fFE")
            );
            assert_eq!(
                transfer.receiver,
                ToPrimitive::cvt("0x4E41e19f939a0040330F7Cd3CFfde8cA96700d9b")
            );
            let value: U256 = ToPrimitive::cvt(
                ethers::utils::parse_ether("0.002312").unwrap(),
            );
            assert_eq!(transfer.asset, Asset::Ether(value));
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
}
