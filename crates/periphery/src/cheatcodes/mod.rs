// A set of cheatcodes that can directly modify the environments

use std::{
    any::type_name,
    collections::{BTreeMap, HashMap},
    fmt::Debug,
};

use crate::{caller::HighLevelCaller, types::SolUint256};
use alloy_json_abi::Function;
use libsofl_core::{
    conversion::ConvertTo,
    engine::{
        state::{BcState, BcStateEditable},
        types::{Address, Bytecode, Bytes, B256, U256},
    },
    error::SoflError,
};

mod inspector;
use alloy_sol_types::SolType;
use inspector::CheatcodeInspector;

mod contract_type;
mod erc20;
mod price_oracle;

#[derive(Debug, Clone)]
enum SlotQueryResult {
    NotFound,
    Found(U256),
}

pub struct CheatCodes {
    // runtime env
    inspector: CheatcodeInspector,

    // slot info: (codehash, calldata) -> slot_state
    slots: BTreeMap<(B256, Bytes), SlotQueryResult>,

    // high-level caller
    caller: HighLevelCaller,
    // abi parser
    // abi_parser: AbiParser,
    abi_cache: HashMap<String, Function>,
}

// fn pack_calldata(fsig: [u8; 4], args: &[Token]) -> Bytes {
//     let args = abi::encode(args);
//     [fsig.as_slice(), args.as_slice()].concat().into()
// }

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
            // abi_parser: AbiParser::default(),
            abi_cache: HashMap::new(),
        }
    }

    pub fn set_caller(mut self, f: &dyn Fn(HighLevelCaller) -> HighLevelCaller) -> Self {
        self.caller = f(self.caller);
        self
    }

    pub fn reset_caller(&mut self) {
        self.caller = HighLevelCaller::default().bypass_check();
    }

    pub fn parse_abi(&mut self, s: &str) -> Result<&Function, SoflError> {
        if !self.abi_cache.contains_key(&s.to_string()) {
            let abi = Function::parse(s)
                .map_err(|e| SoflError::Abi(format!("failed to parse abi: {:?}", e)))?;
            self.abi_cache.insert(s.to_string(), abi);
        }

        Ok(self.abi_cache.get(&s.to_string()).unwrap())
    }

    /// Find the storage slot that is read by executing the given calldata.
    fn find_slot<S>(&mut self, state: &mut S, to: Address, calldata: Bytes) -> Option<U256>
    where
        S: BcState + BcStateEditable,
        S::Error: Debug,
    {
        // staticcall to get the slot, where we force the return type as u256
        self.inspector.reset_access_recording();
        let ret = self
            .caller
            .static_call(state, to, calldata.clone(), &mut self.inspector)
            .ok()?;
        let cdata = SolUint256::abi_decode(&ret, true);
        if cdata.is_err() {
            return None;
        }
        let cdata = cdata.unwrap();

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
                if rdata == cdata {
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
                        .static_call(state, to, calldata.clone(), &mut self.inspector)
                        .ok()?;
                    let cdata = SolUint256::abi_decode(&ret, false);
                    if cdata.is_err() {
                        return None;
                    }
                    let cdata = cdata.unwrap();

                    state
                        .insert_account_storage(to, slot, prev)
                        .expect("insert should not fail");

                    if magic == cdata {
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
    pub fn cheat_read<S>(
        &mut self,
        state: &mut S,
        to: Address,
        calldata: Bytes,
    ) -> Result<Bytes, SoflError>
    where
        S: BcState + BcStateEditable,
        S::Error: Debug,
    {
        // let rtypes: Vec<ParamType> =
        // func.outputs.iter().map(|p| p.kind.clone()).collect();
        // let rtypes = rtypes.as_slice();
        if let Ok(Some(account_info)) = state.basic(to) {
            // let calldata = pack_calldata(func.short_signature(), args);
            let code_hash = account_info.code_hash;
            match self.slots.get(&(code_hash, calldata.clone())) {
                Some(SlotQueryResult::Found(slot)) => {
                    // return self.decode_from_storage(state, to, *slot, rtypes);
                    let v: U256 = state.storage(to, *slot).map_err(|e| {
                        SoflError::BcState(format!("failed to read storage value: {:?}", e))
                    })?;
                    return Ok(v.cvt());
                }
                Some(SlotQueryResult::NotFound) => {}
                None => {
                    // we have not tried to find the slot, so we first try to find the slot
                    if let Some(slot) = self.find_slot(state, to, calldata.clone()) {
                        // cache the slot
                        self.slots
                            .insert((code_hash, calldata), SlotQueryResult::Found(slot));

                        // return self
                        // .decode_from_storage(state, to, slot, rtypes);
                        let v: U256 = state.storage(to, slot).map_err(|e| {
                            SoflError::BcState(format!("failed to read storage value: {:?}", e))
                        })?;
                        return Ok(v.cvt());
                    } else {
                        // we cannnot find the slot, so we cache the result (to avoid trying to
                        // find the slot again)
                        self.slots
                            .insert((code_hash, calldata.clone()), SlotQueryResult::NotFound);
                    }
                }
            }
        }

        self.inspector.disable_access_recording();
        self.caller
            .static_call(state, to, calldata, &mut self.inspector)
    }

    // fn decode_from_storage<E, S>(
    //     &mut self,
    //     state: &mut S,
    //     to: Address,
    //     slot: U256,
    //     rtypes: &[ParamType],
    // ) -> Result<Vec<Token>, SoflError<E>>
    // where
    //     S: BcState,
    // {
    //     let mut rdata = state
    //         .storage(to, slot)
    //         .map_err(SoflError::Db)?
    //         .as_le_bytes()
    //         .to_vec();
    //     rdata.reverse();

    //     abi::decode(rtypes, rdata.as_slice()).map_err(SoflError::Abi)
    // }
}

// cheatcode: cheat_write
impl CheatCodes {
    pub fn cheat_write<S>(
        &mut self,
        state: &mut S,
        to: Address,
        calldata: Bytes,
        data: U256,
    ) -> Result<Option<U256>, SoflError>
    where
        S::Error: Debug,
        S: BcState + BcStateEditable,
    {
        let account_info = state
            .basic(to)
            .map_err(|e| SoflError::BcState(format!("failed to get account basic: {:?}", e)))?
            .ok_or(SoflError::BcState(format!(
                "{}: account does not have code",
                type_name::<Self>()
            )))?;

        // let calldata = pack_calldata(func.short_signature(), args);
        let code_hash = account_info.code_hash;
        match self.slots.get(&(code_hash, calldata.clone())) {
            Some(SlotQueryResult::Found(slot)) => self.write_or_err(state, to, *slot, data),
            Some(SlotQueryResult::NotFound) => Err(SoflError::BcState(format!(
                "{}: cannot find the target slot",
                type_name::<Self>(),
            ))),
            None => {
                // we need to find the slot
                if let Some(slot) = self.find_slot(state, to, calldata.clone()) {
                    // cache the slot
                    self.slots
                        .insert((code_hash, calldata), SlotQueryResult::Found(slot));

                    self.write_or_err(state, to, slot, data)
                } else {
                    // we cannnot find the slot, so we cache the result (to avoid trying to
                    // find the slot again)
                    self.slots
                        .insert((code_hash, calldata), SlotQueryResult::NotFound);
                    Err(SoflError::BcState(format!(
                        "{}: cannot find the target slot",
                        type_name::<Self>()
                    )))
                }
            }
        }
    }

    fn write_or_err<S>(
        &mut self,
        state: &mut S,
        to: Address,
        slot: U256,
        data: U256,
    ) -> Result<Option<U256>, SoflError>
    where
        S::Error: Debug,
        S: BcState + BcStateEditable,
    {
        let rdata = state
            .storage(to, slot)
            .map_err(|e| SoflError::BcState(format!("failed to get storage value: {:?}", e)))?;

        if rdata != data {
            state.insert_account_storage(to, slot, data).map_err(|e| {
                SoflError::BcState(format!("failed to insert account storage: {:?}", e))
            })?;
            Ok(Some(rdata))
        } else {
            Ok(None)
        }
    }
}

// Functions that does not need to access cache
impl CheatCodes {
    pub fn get_balance<S: BcState + BcStateEditable>(
        &mut self,
        state: &mut S,
        account: Address,
    ) -> Result<U256, SoflError>
    where
        S::Error: Debug,
    {
        state
            .basic(account)
            .map_err(|e| SoflError::BcState(format!("failed to get account basic: {:?}", e)))?
            .map_or(Ok(U256::from(0)), |info| Ok(info.balance))
    }

    pub fn get_code_hash<S: BcState + BcStateEditable>(
        &mut self,
        state: &mut S,
        account: Address,
    ) -> Result<B256, SoflError>
    where
        S::Error: Debug,
    {
        state
            .basic(account)
            .map_err(|e| SoflError::BcState(format!("failed to get account basic: {:?}", e)))?
            .map_or(Ok(B256::ZERO), |info| Ok(info.code_hash))
    }

    pub fn get_code<S: BcState + BcStateEditable>(
        &mut self,
        state: &mut S,
        account: Address,
    ) -> Result<Bytecode, SoflError>
    where
        S::Error: Debug,
    {
        if let Some(code) = state
            .basic(account)
            .map_err(|e| SoflError::BcState(format!("failed to get account basic: {:?}", e)))?
            .and_then(|info| info.code)
        {
            Ok(code)
        } else {
            let code_hash = self.get_code_hash(state, account)?;
            state
                .code_by_hash(code_hash)
                .map_err(|e| SoflError::BcState(format!("failed to get code by hash: {:?}", e)))
        }
    }

    pub fn set_balance<S: BcState + BcStateEditable>(
        &mut self,
        state: &mut S,
        address: Address,
        balance: U256,
    ) -> Result<Option<U256>, SoflError>
    where
        S::Error: Debug,
    {
        let mut account_info = state
            .basic(address)
            .map_err(|e| SoflError::BcState(format!("failed to get account basic: {:?}", e)))?
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
mod tests_with_dep {
    use crate::test::get_test_bc_provider;
    use libsofl_core::{
        blockchain::tx_position::TxPosition,
        conversion::ConvertTo,
        engine::types::{Address, U256},
    };

    use super::CheatCodes;

    #[test]
    fn test_get_token_balance() {
        let bp = get_test_bc_provider();

        let fork_at = TxPosition::new(17000001, 0);
        let mut state = bp.bc_state_at(fork_at).unwrap();

        let mut cheatcodes = CheatCodes::new();

        let token: Address = "0xdAC17F958D2ee523a2206206994597C13D831ec7".cvt();
        let account: Address = "0x1497bF2C336EBE4B8745DF52E190Bd0c8129666a".cvt();

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
