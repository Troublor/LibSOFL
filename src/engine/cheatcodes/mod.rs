// A set of cheatcodes that can directly modify the environments

use std::{collections::BTreeMap, fmt::Debug, marker::PhantomData};

use crate::error::SoflError;
use ethers::abi::{self, Function, ParamType, Token};
use reth_primitives::{Address, Bytes, U256};
use revm::{Database, DatabaseCommit};
use revm_primitives::{BlockEnv, CfgEnv, B256};

mod inspector;
use inspector::CheatcodeInspector;

mod erc20;
pub use erc20::ERC20Cheat;

mod price_oracle;
pub use price_oracle::PriceOracleCheat;

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

#[derive(Debug, Default)]
pub struct CheatCodes<S: DatabaseEditable> {
    // phantom
    phantom: PhantomData<S>,

    // runtime env
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
impl<
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    > CheatCodes<S>
{
    pub fn new(mut cfg: CfgEnv, block: BlockEnv) -> Self {
        Self {
            phantom: PhantomData,
            inspector: CheatcodeInspector::default(),
            slots: BTreeMap::new(),
        }
    }

    fn find_slot(
        &mut self,
        state: &mut S,
        to: Address,
        func: &Function,
        args: &[Token],
    ) -> Option<U256> {
        // staticcall to get the slot, where we force the return type as u256
        self.inspector.reset_access_recording();
        let caller = HighLevelCaller::default().bypass_check();
        let ret = caller
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
                    let caller = HighLevelCaller::default().bypass_check();
                    let ret = caller
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
impl<
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    > CheatCodes<S>
{
    // staticcall with slot lookup
    // this function can only work if the target function:
    //  1) is a view function (i.e. does not modify the state)
    //  2) returns a single primitive value (e.g., uint256, address, etc.)
    //  3) is derived from a public storage variable
    pub fn cheat_read(
        &mut self,
        state: &mut S,
        to: Address,
        func: &Function,
        args: &[Token],
    ) -> Result<Vec<Token>, SoflError<E>> {
        let fsig = u32::from_be_bytes(func.short_signature());
        let rtypes: Vec<ParamType> =
            func.outputs.iter().map(|p| p.kind.clone()).collect();
        let rtypes = rtypes.as_slice();
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
                    if let Some(slot) = self.find_slot(state, to, func, args) {
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

        self.inspector.disable_access_recording();
        HighLevelCaller::default().bypass_check().invoke(
            state,
            to,
            func,
            args,
            None,
            &mut self.inspector,
        )
    }

    fn decode_from_storage(
        state: &mut S,
        to: Address,
        slot: U256,
        rtypes: &[ParamType],
    ) -> Result<Vec<Token>, SoflError<E>> {
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
impl<
        E: Debug,
        S: DatabaseEditable<Error = E> + Database<Error = E> + DatabaseCommit,
    > CheatCodes<S>
{
    pub fn cheat_write(
        &mut self,
        state: &mut S,
        to: Address,
        func: &Function,
        args: &[Token],
        data: U256,
    ) -> Result<Option<U256>, SoflError<E>> {
        let fsig = u32::from_be_bytes(func.short_signature());
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
                if let Some(slot) = self.find_slot(state, to, func, args) {
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

    fn write_or_err(
        state: &mut S,
        to: Address,
        slot: U256,
        data: U256,
    ) -> Result<Option<U256>, SoflError<E>> {
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
impl<S: DatabaseEditable + Database> CheatCodes<S> {
    pub fn get_balance(
        &self,
        state: &mut S,
        account: Address,
    ) -> Result<U256, SoflError<<S as Database>::Error>> {
        state
            .basic(account)
            .map_err(SoflError::Db)?
            .map_or(Ok(U256::from(0)), |info| Ok(info.balance))
    }

    pub fn set_balance(
        &self,
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
    use revm_primitives::{BlockEnv, CfgEnv, U256};

    use crate::engine::cheatcodes::ERC20Cheat;
    use crate::engine::state::state::BcStateBuilder;
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

        let mut cheatcode =
            CheatCodes::new(CfgEnv::default(), BlockEnv::default());

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
