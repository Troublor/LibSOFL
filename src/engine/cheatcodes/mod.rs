// A set of cheatcodes that can directly modify the environments

use crate::{engine::state::BcState, error::SoflError};
use ethers::abi::{self, ParamType, Token};
use reth_primitives::{Address, Bytes, U256};
use revm::{Database, Inspector, EVM};
use revm_primitives::{
    BlockEnv, CfgEnv, Env, ResultAndState, TransactTo, TxEnv,
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

#[derive(Debug, Default)]
pub struct CheatCodes {
    // need add some cache here
    env: Env,
    inspector: CheatcodeInspector,
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
            inspector: CheatcodeInspector::default(),
        }
    }

    fn staticcall<'a, 'b: 'a, S: BcState + 'b>(
        &'a mut self,
        state: &'b mut S,
        to: Address,
        fsig: u32,
        args: &[Token],
        rtypes: &[ParamType],
    ) -> Result<Vec<Token>, SoflError<S::Err>> {
        let fsig = fsig.to_be_bytes();
        let args = abi::encode(args);
        let data = [fsig.as_slice(), args.as_slice()].concat().into();

        let result = self.call(state, Some(to), Some(data))?;
        match result.result {
            revm_primitives::ExecutionResult::Success {
                output: revm_primitives::Output::Call(bytes),
                ..
            } => abi::decode(rtypes, &bytes).map_err(SoflError::Abi),
            _ => Err(SoflError::Exec(result.result)),
        }
    }

    fn call<'a, 'b: 'a, S: BcState + 'b>(
        &'a mut self,
        state: &'b mut S,
        to: Option<Address>,
        data: Option<Bytes>,
    ) -> Result<ResultAndState, SoflError<S::Err>> {
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

    fn find_slot<'a, 'b: 'a, S: BcState + 'b>(
        &'a mut self,
        state: &'b mut S,
        to: Address,
        fsig: u32,
        args: &[Token],
    ) -> Option<U256> {
        // enable inspector to get the slot
        self.inspector.toggle_access_recording(true);

        // staticcall to get the slot, where we force the return type as u256
        let ret = self
            .staticcall(state, to, fsig, args, &[ParamType::Uint(256)])
            .ok()?;
        let cdata = get_the_first_uint!(ret);

        // check the access
        if let Some(ref accesses) = self.inspector.accesses {
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

                    // update the target slot
                    state
                        .insert_account_storage(to, slot, magic)
                        .expect("insert should not fail");

                    let ret = self
                        .staticcall(
                            state,
                            to,
                            fsig,
                            args,
                            &[ParamType::Uint(256)],
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

// cheatcodes
impl CheatCodes {
    pub fn get_token_balance<S: BcState>(
        &mut self,
        state: &mut S,
        token: Address,
        account: Address,
    ) -> Result<U256, SoflError<S::Err>> {
        // enable inspector to get the slot
        self.inspector.toggle_access_recording(true);

        println!(
            "{:?}",
            self.find_slot(
                state,
                token,
                0x70a08231u32,
                &[Token::Address(account.into())]
            )
        );

        // signature: balanceOf(address) -> 0x70a08231
        let result = self.staticcall(
            state,
            token,
            0x70a08231u32,
            &[Token::Address(account.into())],
            &[ParamType::Uint(256)],
        )?;

        Ok(result[0].clone().into_uint().expect("cannot fail").into())
    }
}
