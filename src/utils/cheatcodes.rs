// A set of cheatcodes that can directly modify the environments

use crate::{
    engine::state::{self, BcState},
    error::SoflError,
};
use ethers::abi::{self, ParamType, Token};
use reth_primitives::{Address, Bytes, U256};
use revm::{Inspector, EVM};
use revm_primitives::{
    BlockEnv, CfgEnv, Env, ResultAndState, TransactTo, TxEnv,
};

#[derive(Debug, Default)]
pub struct CheatCodes {
    // need add some cache here
    env: Env,
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
        }
    }

    fn staticcall<'a, 'b: 'a, S: BcState + 'b, I: Inspector<&'b mut S>>(
        &'a mut self,
        state: &'b mut S,
        to: Address,
        fsig: u32,
        args: &[Token],
        rtypes: &[ParamType],
        inspector: I,
    ) -> Result<Vec<Token>, SoflError<S::Error>> {
        let fsig = fsig.to_be_bytes();
        let args = abi::encode(args);
        let data = [fsig.as_slice(), args.as_slice()].concat().into();

        let result = self.call(state, Some(to), Some(data), inspector)?;
        match result.result {
            revm_primitives::ExecutionResult::Success {
                output: revm_primitives::Output::Call(bytes),
                ..
            } => abi::decode(rtypes, &bytes).map_err(SoflError::Abi),
            _ => Err(SoflError::Exec(result.result)),
        }
    }

    fn call<'a, 'b: 'a, S: BcState + 'b, I: Inspector<&'b mut S>>(
        &'a mut self,
        state: &'b mut S,
        to: Option<Address>,
        data: Option<Bytes>,
        inspector: I,
    ) -> Result<ResultAndState, SoflError<S::Error>> {
        self.fill_tx_env_for_call(to, data);

        let mut evm: EVM<&mut S> = revm::EVM::with_env(self.env.clone());
        evm.database(state);

        S::transact_with_tx_filled(&mut evm, inspector)
    }

    // fill a call transaction with the given data
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
}

// cheatcodes
impl CheatCodes {
    pub fn get_token_balance<S: BcState>(
        &mut self,
        state: &mut S,
        token: Address,
        account: Address,
    ) -> Result<U256, SoflError<S::Error>> {
        // signature: balanceOf(address)
        let result = self.staticcall(
            state,
            token,
            0x70a08231u32,
            &[Token::Address(account.into())],
            &[ParamType::Uint(256)],
            state::no_inspector(),
        )?;

        Ok(result[0].clone().into_uint().expect("cannot fail").into())
    }
}
