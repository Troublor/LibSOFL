use reth_primitives::{Address, Log};
use revm::Inspector;
use revm_primitives::U256;
use serde::{Deserialize, Serialize};

use crate::{
    engine::state::BcState,
    utils::{
        abi::{ERC1155_ABI, ERC20_ABI, ERC721_ABI, ERC777_ABI, WETH_ABI},
        conversion::{Convert, ToEthers, ToPrimitive},
    },
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
pub struct Transfer {
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

impl Transfer {
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
        // } else if let Ok(mut parsed_log) = WETH_ABI. {
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
             else {return None};
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoneyFlowInspector {
    /// The money flow in each transaction.
    /// Each transaction is represented as a list of transfers.
    transfers: Vec<Vec<Transfer>>,
}

impl<BS: BcState> Inspector<BS> for MoneyFlowInspector {}

impl<BS: BcState> super::MultiTxInspector<BS> for MoneyFlowInspector {}
