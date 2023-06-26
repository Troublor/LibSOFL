// A set of cheatcodes that can directly modify the environments

use std::collections::BTreeMap;

use crate::{engine::state::BcState, error::SoflError};
use ethers::abi::{self, ParamType, Token};
use reth_primitives::{Address, Bytes, U256};
use revm::EVM;
use revm_primitives::{
    BlockEnv, CfgEnv, Env, ResultAndState, TransactTo, TxEnv, B256,
};

mod inspector;
use inspector::CheatcodeInspector;

macro_rules! get_the_first_uint {
    ($tokens:expr) => {
        if $tokens.len() != 1 {
            return None;
        } else if let Some(Token::Uint(uint)) = $tokens.get(0) {
            *uint
        } else {
            return None;
        }
    };
}

#[derive(Debug, Clone)]
enum SlotQueryResult {
    NotFound,
    Found(U256),
}

#[derive(Debug, Default)]
pub struct CheatCodes {
    // runtime env
    env: Env,
    inspector: CheatcodeInspector,

    // slot info: (codehash, calldata) -> slot_state
    slots: BTreeMap<(B256, Bytes), SlotQueryResult>,
}

fn pack_calldata(fsig: u32, args: &[Token]) -> Bytes {
    let fsig = fsig.to_be_bytes();
    let args = abi::encode(args);
    [fsig.as_slice(), args.as_slice()].concat().into()
}

// basic functionality
impl CheatCodes {
    pub fn new(mut cfg: CfgEnv, block: BlockEnv) -> Self {
        // we want to disable this in eth_call, since this is common practice used by other node
        // impls and providers <https://github.com/foundry-rs/foundry/issues/4388>
        cfg.disable_block_gas_limit = true;

        // Disabled because eth_call is sometimes used with eoa senders
        // See <https://github.com/paradigmxyz/reth/issues/1959>
        cfg.disable_eip3607 = true;

        // The basefee should be ignored for eth_call
        // See:
        // <https://github.com/ethereum/go-ethereum/blob/ee8e83fa5f6cb261dad2ed0a7bbcde4930c41e6c/internal/ethapi/api.go#L985>
        cfg.disable_base_fee = true;

        Self {
            env: Env {
                cfg,
                block,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    pub fn call<S: BcState>(
        &mut self,
        state: &mut S,
        to: Address,
        fsig: u32,
        args: &[Token],
        rtypes: &[ParamType],
        force_tracing: Option<bool>,
    ) -> Result<Vec<Token>, SoflError<S::DbErr>> {
        match force_tracing {
            Some(true) => self.inspector.reset_access_recording(),
            Some(false) => self.inspector.disable_access_recording(),
            None => (),
        }

        let data = pack_calldata(fsig, args);

        let result = self.low_level_call(state, Some(to), Some(data))?;
        match result.result {
            revm_primitives::ExecutionResult::Success {
                output: revm_primitives::Output::Call(bytes),
                ..
            } => abi::decode(rtypes, &bytes).map_err(SoflError::Abi),
            _ => Err(SoflError::Exec(result.result)),
        }
    }

    fn low_level_call<S: BcState>(
        &mut self,
        state: &mut S,
        to: Option<Address>,
        data: Option<Bytes>,
    ) -> Result<ResultAndState, SoflError<S::DbErr>> {
        self.fill_tx_env_for_call(to, data);

        let mut evm: EVM<&mut S> = revm::EVM::with_env(self.env.clone());
        evm.database(state);

        S::transact_with_tx_filled(&mut evm, &mut self.inspector)
    }

    // fill the tx env for an eth_call
    fn fill_tx_env_for_call(
        &mut self,
        to: Option<Address>,
        data: Option<Bytes>,
    ) {
        self.env.tx = TxEnv {
            gas_limit: u64::MAX,
            nonce: None,
            gas_price: U256::ZERO,
            gas_priority_fee: None,
            transact_to: to
                .map(TransactTo::Call)
                .unwrap_or_else(TransactTo::create),
            data: data.map(|data| data.0).unwrap_or_default(),
            chain_id: None,
            ..Default::default()
        };
    }

    fn find_slot<S: BcState>(
        &mut self,
        state: &mut S,
        to: Address,
        fsig: u32,
        args: &[Token],
    ) -> Option<U256> {
        // staticcall to get the slot, where we force the return type as u256
        let ret = self
            .call(state, to, fsig, args, &[ParamType::Uint(256)], Some(true))
            .ok()?;
        let cdata = get_the_first_uint!(ret);

        // check the access
        if let Some(ref accesses) = self.inspector.accesses {
            // check whether it is a real staticcall
            if !accesses.writes.is_empty() {
                return None;
            }

            // check read accesses
            let raccesses = accesses.reads.get(&to)?.clone();

            if raccesses.len() == 1 {
                let slot = raccesses[0];

                // sanity check
                let rdata = state.storage(to, slot).ok()?;
                if rdata == cdata.into() {
                    return Some(slot);
                }
            } else {
                // there are multiple reads, we need to check if the data is the same
                let magic = U256::from(0xdeadbeefu64);
                for slot in raccesses {
                    let prev = state.storage(to, slot).ok()?;
                    if cdata != prev.into() {
                        continue;
                    }

                    // update the target slot
                    state
                        .insert_account_storage(to, slot, magic)
                        .expect("insert should not fail");

                    // we have to do another call to check if the slot is correct,
                    // because changing the slot might change the program flow
                    let ret = self
                        .call(
                            state,
                            to,
                            fsig,
                            args,
                            &[ParamType::Uint(256)],
                            Some(false),
                        )
                        .ok()?;
                    let cdata = get_the_first_uint!(ret);

                    state
                        .insert_account_storage(to, slot, prev)
                        .expect("insert should not fail");

                    if magic == cdata.into() {
                        // we got the slot!
                        return Some(slot);
                    }
                }
            }
        }

        None
    }
}

// cheatcode: cheat_read
impl CheatCodes {
    // staticcall with slot lookup
    // this function can only work if the target function:
    //  1) is a view function (i.e. does not modify the state)
    //  2) returns a single primitive value (e.g., uint256, address, etc.)
    //  3) is derived from a public storage variable
    pub fn cheat_read<S: BcState>(
        &mut self,
        state: &mut S,
        to: Address,
        fsig: u32,
        args: &[Token],
        rtypes: &[ParamType],
    ) -> Result<Vec<Token>, SoflError<S::DbErr>> {
        if let Ok(Some(account_info)) = state.basic(to) {
            let calldata = pack_calldata(fsig, args);
            let code_hash = account_info.code_hash;
            match self.slots.get(&(code_hash, calldata.clone())) {
                Some(SlotQueryResult::Found(slot)) => {
                    return Self::decode_from_storage(state, to, *slot, rtypes);
                }
                Some(SlotQueryResult::NotFound) => {}
                None => {
                    // we have not tried to find the slot, so we first try to find the slot
                    if let Some(slot) = self.find_slot(state, to, fsig, args) {
                        // cache the slot
                        self.slots.insert(
                            (code_hash, calldata),
                            SlotQueryResult::Found(slot),
                        );

                        return Self::decode_from_storage(
                            state, to, slot, rtypes,
                        );
                    } else {
                        // we cannnot find the slot, so we cache the result (to avoid trying to
                        // find the slot again)
                        self.slots.insert(
                            (code_hash, calldata),
                            SlotQueryResult::NotFound,
                        );
                    }
                }
            }
        }

        self.call(state, to, fsig, args, rtypes, Some(false))
    }

    fn decode_from_storage<'a, S: BcState + 'a>(
        state: &mut S,
        to: Address,
        slot: U256,
        rtypes: &[ParamType],
    ) -> Result<Vec<Token>, SoflError<S::DbErr>> {
        let mut rdata = state
            .storage(to, slot)
            .map_err(SoflError::Db)?
            .as_le_bytes()
            .to_vec();
        rdata.reverse();

        abi::decode(rtypes, rdata.as_slice()).map_err(SoflError::Abi)
    }
}

// cheatcode: cheat_write
impl CheatCodes {
    pub fn cheat_write<S: BcState>(
        &mut self,
        state: &mut S,
        to: Address,
        fsig: u32,
        args: &[Token],
        data: U256,
    ) -> Result<Option<U256>, SoflError<S::DbErr>> {
        let account_info = state.basic(to).map_err(SoflError::Db)?.ok_or(
            SoflError::Custom("account does not have code".to_string()),
        )?;

        let calldata = pack_calldata(fsig, args);
        let code_hash = account_info.code_hash;
        match self.slots.get(&(code_hash, calldata.clone())) {
            Some(SlotQueryResult::Found(slot)) => {
                Self::write_or_err(state, to, *slot, data)
            }
            Some(SlotQueryResult::NotFound) => Err(SoflError::Custom(
                "cannot find the target slot".to_string(),
            )),
            None => {
                // we need to find the slot
                if let Some(slot) = self.find_slot(state, to, fsig, args) {
                    // cache the slot
                    self.slots.insert(
                        (code_hash, calldata),
                        SlotQueryResult::Found(slot),
                    );

                    Self::write_or_err(state, to, slot, data)
                } else {
                    // we cannnot find the slot, so we cache the result (to avoid trying to
                    // find the slot again)
                    self.slots.insert(
                        (code_hash, calldata),
                        SlotQueryResult::NotFound,
                    );
                    Err(SoflError::Custom(
                        "cannot find the target slot".to_string(),
                    ))
                }
            }
        }
    }

    fn write_or_err<S: BcState>(
        state: &mut S,
        to: Address,
        slot: U256,
        data: U256,
    ) -> Result<Option<U256>, SoflError<S::DbErr>> {
        let rdata = state.storage(to, slot).map_err(SoflError::Db)?;

        if rdata != data {
            state
                .insert_account_storage(to, slot, data)
                .map_err(SoflError::Db)?;
            Ok(Some(rdata))
        } else {
            Ok(None)
        }
    }
}

// cheatcodes: get functions
impl CheatCodes {
    pub fn get_balance<S: BcState>(
        &self,
        state: &mut S,
        account: Address,
    ) -> Result<U256, SoflError<S::DbErr>> {
        state
            .basic(account)
            .map_err(SoflError::Db)?
            .map_or(Ok(U256::from(0)), |info| Ok(info.balance))
    }

    pub fn get_erc20_balance<S: BcState>(
        &mut self,
        state: &mut S,
        token: Address,
        account: Address,
    ) -> Result<U256, SoflError<S::DbErr>> {
        // signature: balanceOf(address) -> 0x70a08231
        let result = self.cheat_read(
            state,
            token,
            0x70a08231u32,
            &[Token::Address(account.into())],
            &[ParamType::Uint(256)],
        )?;

        Ok(result[0].clone().into_uint().expect("cannot fail").into())
    }

    pub fn get_erc20_total_supply<S: BcState>(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<U256, SoflError<S::DbErr>> {
        // signature: totalSupply() -> 0x18160ddd
        let result = self.cheat_read(
            state,
            token,
            0x18160dddu32,
            &[],
            &[ParamType::Uint(256)],
        )?;

        Ok(result[0].clone().into_uint().expect("cannot fail").into())
    }

    pub fn get_erc20_decimals<S: BcState>(
        &mut self,
        state: &mut S,
        token: Address,
    ) -> Result<U256, SoflError<S::DbErr>> {
        // signature: decimals() -> 0x313ce567
        let result = self.cheat_read(
            state,
            token,
            0x313ce567u32,
            &[],
            &[ParamType::Uint(256)],
        )?;

        Ok(result[0].clone().into_uint().expect("cannot fail").into())
    }
}

// cheatcodes: set functions
impl CheatCodes {
    pub fn set_balance<S: BcState>(
        &self,
        state: &mut S,
        address: Address,
        balance: U256,
    ) -> Result<Option<U256>, SoflError<S::DbErr>> {
        let mut account_info = state
            .basic(address)
            .map_err(SoflError::Db)?
            .unwrap_or_default();
        let old_balance = account_info.balance;

        if old_balance == balance {
            return Ok(None);
        }

        account_info.balance = balance;
        state.insert_account_info(address, account_info);

        Ok(Some(old_balance))
    }

    // return the old balance if updated
    pub fn set_erc20_balance<S: BcState>(
        &mut self,
        state: &mut S,
        token: Address,
        account: Address,
        balance: U256,
    ) -> Result<Option<U256>, SoflError<S::DbErr>> {
        // signature: balanceOf(address) -> 0x70a08231
        if let Some(old_balance) = self.cheat_write(
            state,
            token,
            0x70a08231u32,
            &[Token::Address(account.into())],
            balance,
        )? {
            // we need to update total supply
            let total_supply = self.get_erc20_total_supply(state, token)?;

            // signature: totalSupply() -> 0x18160ddd
            self.cheat_write(
                state,
                token,
                0x18160dddu32,
                &[],
                total_supply + balance - old_balance,
            )?;

            Ok(Some(old_balance))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests_with_db {
    use std::{path::Path, str::FromStr};

    use reth_primitives::Address;
    use revm_primitives::U256;

    use crate::{
        config::flags::SoflConfig,
        engine::{
            providers::BcProviderBuilder, state::fork::ForkedBcState,
            transaction::TxPosition,
        },
    };

    use super::CheatCodes;

    #[test]
    fn test_get_token_balance() {
        let datadir = SoflConfig::load().unwrap().reth.datadir;
        let datadir = Path::new(&datadir);
        let bp = BcProviderBuilder::with_mainnet_reth_db(datadir).unwrap();

        let fork_at = TxPosition::new(17000001, 0);
        let mut state = ForkedBcState::fork_at(&bp, fork_at).unwrap();

        let mut cheatcode = CheatCodes::default();

        let token =
            Address::from_str("0xdAC17F958D2ee523a2206206994597C13D831ec7")
                .unwrap();
        let account =
            Address::from_str("0x1497bF2C336EBE4B8745DF52E190Bd0c8129666a")
                .unwrap();

        let balance1 = cheatcode
            .get_erc20_balance(&mut state, token, account)
            .unwrap();

        let balance2 = cheatcode
            .get_erc20_balance(&mut state, token, account)
            .unwrap();

        assert_eq!(balance1, U256::from(1299267380));
        assert_eq!(balance2, U256::from(1299267380));
    }
}
