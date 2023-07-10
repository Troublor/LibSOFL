// A set of cheatcodes that can directly modify the environments

use std::{collections::BTreeMap, fmt::Debug};

use crate::error::SoflError;
use ethers::abi::{self, Function, ParamType, Token};
use reth_primitives::{Address, Bytes, U256};
use revm::{Database, DatabaseCommit};
use revm_primitives::{Bytecode, B256};

mod inspector;
use inspector::CheatcodeInspector;

mod erc20;
mod price_oracle;

mod contract_type;
pub use contract_type::ContractType;

use super::{state::DatabaseEditable, utils::HighLevelCaller};

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

#[derive(Debug)]
pub struct CheatCodes {
    // runtime env
    inspector: CheatcodeInspector,

    // slot info: (codehash, calldata) -> slot_state
    slots: BTreeMap<(B256, Bytes), SlotQueryResult>,

    // high-level caller
    caller: HighLevelCaller,
}

fn pack_calldata(fsig: [u8; 4], args: &[Token]) -> Bytes {
    let args = abi::encode(args);
    [fsig.as_slice(), args.as_slice()].concat().into()
}

impl Default for CheatCodes {
    fn default() -> Self {
        Self::new()
    }
}

// basic functionality
impl CheatCodes {
    pub fn new() -> Self {
        Self {
            caller: HighLevelCaller::default().bypass_check(),
            inspector: CheatcodeInspector::default(),
            slots: BTreeMap::new(),
        }
    }

    fn find_slot<E, S>(
        &mut self,
        state: &mut S,
        to: Address,
        func: &Function,
        args: &[Token],
    ) -> Option<U256>
    where
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    {
        // staticcall to get the slot, where we force the return type as u256
        self.inspector.reset_access_recording();
        let ret = self
            .caller
            .invoke(state, to, func, args, None, &mut self.inspector)
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
                    self.inspector.disable_access_recording();
                    let ret = self
                        .caller
                        .invoke(
                            state,
                            to,
                            func,
                            args,
                            None,
                            &mut self.inspector,
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
    pub fn cheat_read<E, S>(
        &mut self,
        state: &mut S,
        to: Address,
        func: &Function,
        args: &[Token],
    ) -> Result<Vec<Token>, SoflError<E>>
    where
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    {
        let rtypes: Vec<ParamType> =
            func.outputs.iter().map(|p| p.kind.clone()).collect();
        let rtypes = rtypes.as_slice();
        if let Ok(Some(account_info)) = state.basic(to) {
            let calldata = pack_calldata(func.short_signature(), args);
            let code_hash = account_info.code_hash;
            match self.slots.get(&(code_hash, calldata.clone())) {
                Some(SlotQueryResult::Found(slot)) => {
                    return self.decode_from_storage(state, to, *slot, rtypes);
                }
                Some(SlotQueryResult::NotFound) => {}
                None => {
                    // we have not tried to find the slot, so we first try to find the slot
                    if let Some(slot) = self.find_slot(state, to, func, args) {
                        // cache the slot
                        self.slots.insert(
                            (code_hash, calldata),
                            SlotQueryResult::Found(slot),
                        );

                        return self
                            .decode_from_storage(state, to, slot, rtypes);
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

        self.inspector.disable_access_recording();
        self.caller.view(state, to, func, args, &mut self.inspector)
    }

    fn decode_from_storage<E, S>(
        &mut self,
        state: &mut S,
        to: Address,
        slot: U256,
        rtypes: &[ParamType],
    ) -> Result<Vec<Token>, SoflError<E>>
    where
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E>,
    {
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
    pub fn cheat_write<E, S>(
        &mut self,
        state: &mut S,
        to: Address,
        func: &Function,
        args: &[Token],
        data: U256,
    ) -> Result<Option<U256>, SoflError<E>>
    where
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    {
        let account_info = state.basic(to).map_err(SoflError::Db)?.ok_or(
            SoflError::Custom("account does not have code".to_string()),
        )?;

        let calldata = pack_calldata(func.short_signature(), args);
        let code_hash = account_info.code_hash;
        match self.slots.get(&(code_hash, calldata.clone())) {
            Some(SlotQueryResult::Found(slot)) => {
                self.write_or_err(state, to, *slot, data)
            }
            Some(SlotQueryResult::NotFound) => Err(SoflError::Custom(
                "cannot find the target slot".to_string(),
            )),
            None => {
                // we need to find the slot
                if let Some(slot) = self.find_slot(state, to, func, args) {
                    // cache the slot
                    self.slots.insert(
                        (code_hash, calldata),
                        SlotQueryResult::Found(slot),
                    );

                    self.write_or_err(state, to, slot, data)
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

    fn write_or_err<E, S>(
        &mut self,
        state: &mut S,
        to: Address,
        slot: U256,
        data: U256,
    ) -> Result<Option<U256>, SoflError<E>>
    where
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E>,
    {
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

// Functions that does not need to access cache
impl CheatCodes {
    pub fn get_balance<S: DatabaseEditable + Database>(
        &mut self,
        state: &mut S,
        account: Address,
    ) -> Result<U256, SoflError<<S as Database>::Error>> {
        state
            .basic(account)
            .map_err(SoflError::Db)?
            .map_or(Ok(U256::from(0)), |info| Ok(info.balance))
    }

    pub fn get_code_hash<S: DatabaseEditable + Database>(
        &mut self,
        state: &mut S,
        account: Address,
    ) -> Result<B256, SoflError<<S as Database>::Error>> {
        state
            .basic(account)
            .map_err(SoflError::Db)?
            .map_or(Ok(B256::zero()), |info| Ok(info.code_hash))
    }

    pub fn get_code<S: DatabaseEditable + Database>(
        &mut self,
        state: &mut S,
        account: Address,
    ) -> Result<Bytecode, SoflError<<S as Database>::Error>> {
        if let Some(code) = state
            .basic(account)
            .map_err(SoflError::Db)?
            .and_then(|info| info.code)
        {
            Ok(code)
        } else {
            let code_hash = self.get_code_hash(state, account)?;
            state.code_by_hash(code_hash).map_err(SoflError::Db)
        }
    }

    pub fn set_balance<S: DatabaseEditable + Database>(
        &mut self,
        state: &mut S,
        address: Address,
        balance: U256,
    ) -> Result<Option<U256>, SoflError<<S as Database>::Error>> {
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
}

#[cfg(test)]
mod tests_with_db {
    use std::{path::Path, str::FromStr};

    use reth_primitives::Address;
    use revm_primitives::U256;

    use crate::engine::state::BcStateBuilder;
    use crate::{
        config::flags::SoflConfig,
        engine::{
            providers::BcProviderBuilder, transactions::position::TxPosition,
        },
    };

    use super::CheatCodes;

    #[test]
    fn test_get_token_balance() {
        let datadir = SoflConfig::load().unwrap().reth.datadir;
        let datadir = Path::new(&datadir);
        let bp = BcProviderBuilder::with_mainnet_reth_db(datadir).unwrap();

        let fork_at = TxPosition::new(17000001, 0);
        let mut state = BcStateBuilder::fork_at(&bp, fork_at).unwrap();

        let mut cheatcodes = CheatCodes::new();

        let token =
            Address::from_str("0xdAC17F958D2ee523a2206206994597C13D831ec7")
                .unwrap();
        let account =
            Address::from_str("0x1497bF2C336EBE4B8745DF52E190Bd0c8129666a")
                .unwrap();

        let balance1 = cheatcodes
            .get_erc20_balance(&mut state, token, account)
            .unwrap();

        let balance2 = cheatcodes
            .get_erc20_balance(&mut state, token, account)
            .unwrap();

        assert_eq!(balance1, U256::from(1299267380));
        assert_eq!(balance2, U256::from(1299267380));
    }
}

#[cfg(test)]
mod tests_with_dep {
    use std::str::FromStr;

    use reth_primitives::Address;
    use revm_primitives::U256;

    use crate::engine::state::BcStateBuilder;
    use crate::engine::transactions::position::TxPosition;
    use crate::utils::testing::get_testing_bc_provider;

    use super::CheatCodes;

    #[test]
    fn test_get_token_balance() {
        let bp = get_testing_bc_provider();

        let fork_at = TxPosition::new(17000001, 0);
        let mut state = BcStateBuilder::fork_at(&bp, fork_at).unwrap();

        let mut cheatcodes = CheatCodes::new();

        let token =
            Address::from_str("0xdAC17F958D2ee523a2206206994597C13D831ec7")
                .unwrap();
        let account =
            Address::from_str("0x1497bF2C336EBE4B8745DF52E190Bd0c8129666a")
                .unwrap();

        let balance1 = cheatcodes
            .get_erc20_balance(&mut state, token, account)
            .unwrap();

        let balance2 = cheatcodes
            .get_erc20_balance(&mut state, token, account)
            .unwrap();

        assert_eq!(balance1, U256::from(1299267380));
        assert_eq!(balance2, U256::from(1299267380));
    }
}
