// serde counterpart of structs in revm crate

use std::ops::Range;

use serde::{ser::SerializeStruct, Deserialize, Serialize};

use crate::engine::types::{
    Address, CallFrame, CallOutcome, Context, CreateFrame, CreateOutcome,
    Database, Env, EvmContext, Frame, FrameData, FrameResult, Interpreter,
    InterpreterResult, JournalCheckpoint, JournaledState, PrecompileSpecId,
    Precompiles, SpecId,
};

use super::interpreter::{InterpreterResultSerde, InterpreterSerde};

/// SubRoutine checkpoint that will help us to go back from this.
/// Serde counterpart of JournalCheckpoint struct in revm crate.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(remote = "JournalCheckpoint")]
pub struct JournalCheckpointSerde {
    #[serde(getter = "get_journal_checkpoint_log_i")]
    log_i: usize,

    #[serde(getter = "get_journal_checkpoint_journal_i")]
    journal_i: usize,
}

fn get_journal_checkpoint_log_i(jc: &JournalCheckpoint) -> usize {
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    struct JournalCheckpointTwin {
        log_i: usize,
        journal_i: usize,
    }
    let jct: JournalCheckpointTwin = unsafe { std::mem::transmute(jc.clone()) };
    jct.log_i
}

fn get_journal_checkpoint_journal_i(jc: &JournalCheckpoint) -> usize {
    #[derive(Debug, Copy, Clone, PartialEq, Eq)]
    struct JournalCheckpointTwin {
        log_i: usize,
        journal_i: usize,
    }
    let jct: JournalCheckpointTwin = unsafe { std::mem::transmute(jc.clone()) };
    jct.journal_i
}

impl From<JournalCheckpointSerde> for JournalCheckpoint {
    fn from(jc: JournalCheckpointSerde) -> Self {
        let jct: JournalCheckpoint = unsafe { std::mem::transmute(jc) };
        jct
    }
}

/// Serde counterpart of FrameData struct in revm crate
#[derive(Debug, Serialize, Deserialize)]
#[serde(remote = "FrameData")]
pub struct FrameDataSerde {
    /// Journal checkpoint
    #[serde(with = "JournalCheckpointSerde")]
    pub checkpoint: JournalCheckpoint,
    /// Interpreter
    #[serde(with = "InterpreterSerde")]
    pub interpreter: Interpreter,
}

/// Call CallStackFrame.
/// Serde counterpart of CallFrame struct in revm crate
#[derive(Debug, Serialize, Deserialize)]
#[serde(remote = "CallFrame")]
pub struct CallFrameSerde {
    /// Call frame has return memory range where output will be stored.
    pub return_memory_range: Range<usize>,
    /// Frame data
    #[serde(with = "FrameDataSerde")]
    pub frame_data: FrameData,
}

/// Serde counterpart of CreateFrame struct in revm crate
#[derive(Debug, Serialize, Deserialize)]
#[serde(remote = "CreateFrame")]
pub struct CreateFrameSerde {
    /// Create frame has a created address.
    pub created_address: Address,
    /// Frame data
    #[serde(with = "FrameDataSerde")]
    pub frame_data: FrameData,
}

/// Call stack frame.
/// Serde counterpart of Frame enum in revm crate
#[derive(Debug, Serialize)]
#[serde(remote = "Frame")]
pub enum FrameSerde {
    Call(#[serde(with = "CallFrameSerde")] Box<CallFrame>),
    Create(#[serde(with = "CreateFrameSerde")] Box<CreateFrame>),
}

impl<'de> FrameSerde {
    pub fn deserialize<D>(deserializer: D) -> Result<Frame, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        enum FrameTwin {
            Call(#[serde(with = "CallFrameSerde")] CallFrame),
            Create(#[serde(with = "CreateFrameSerde")] CreateFrame),
        }
        let frame = FrameTwin::deserialize(deserializer)?;
        match frame {
            FrameTwin::Call(call_frame) => {
                Ok(Frame::Call(Box::new(call_frame)))
            }
            FrameTwin::Create(create_frame) => {
                Ok(Frame::Create(Box::new(create_frame)))
            }
        }
    }
}

/// Represents the outcome of a call operation in a virtual machine.
/// Serde counterpart of CallOutcome struct in revm crate.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(remote = "CallOutcome")]
pub struct CallOutcomeSerde {
    #[serde(with = "InterpreterResultSerde")]
    pub result: InterpreterResult,
    pub memory_offset: Range<usize>,
}

/// Represents the outcome of a create operation in an interpreter.
/// Serde counterpart of CreateOutcome struct in revm crate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(remote = "CreateOutcome")]
pub struct CreateOutcomeSerde {
    // The result of the interpreter operation.
    #[serde(with = "InterpreterResultSerde")]
    pub result: InterpreterResult,
    // An optional address associated with the create operation.
    pub address: Option<Address>,
}

/// Serde counterpart of FrameResult enum in revm crate.
#[derive(Serialize, Deserialize)]
#[serde(remote = "FrameResult")]
pub enum FrameResultSerde {
    Call(#[serde(with = "CallOutcomeSerde")] CallOutcome),
    Create(#[serde(with = "CreateOutcomeSerde")] CreateOutcome),
}

/// EVM contexts contains data that EVM needs for execution.
/// Serde counterpart of EvmContext struct in revm crate.
#[derive(Debug)]
pub struct EvmContextSerde<DB: Database> {
    /// EVM Environment contains all the information about config, block and transaction that
    /// evm needs.
    pub env: Box<Env>,
    /// EVM State with journaling support.
    pub journaled_state: JournaledState,
    /// Database to load data from.
    pub db: DB,
}

pub struct EvmContextTwin<DB: Database> {
    pub spec_id: SpecId,
    pub env: Box<Env>,
    pub journaled_state: JournaledState,
    pub db: DB,
}

impl<DB: Database + Serialize> EvmContextSerde<DB> {
    pub fn serialize<S>(
        _self: &EvmContext<DB>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut ctx = serializer.serialize_struct("EvmContext", 4)?;
        ctx.serialize_field("spec_id", &_self.spec_id())?;
        ctx.serialize_field("env", &_self.env)?;
        ctx.serialize_field("journaled_state", &_self.journaled_state)?;
        ctx.serialize_field("db", &_self.db)?;
        ctx.end()
    }
}

impl<'de, DB: Database + Deserialize<'de>> EvmContextSerde<DB> {
    pub fn deserialize<D>(deserializer: D) -> Result<EvmContext<DB>, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct EvmContextTwin<DB: Database> {
            spec_id: SpecId,
            env: Env,
            journaled_state: JournaledState,
            db: DB,
        }
        let twin: EvmContextTwin<DB> =
            EvmContextTwin::deserialize(deserializer)?;
        let ctx = EvmContext {
            env: Box::new(twin.env),
            journaled_state: twin.journaled_state,
            db: twin.db,
            error: None,
            precompiles: Precompiles::new(PrecompileSpecId::from_spec_id(
                twin.spec_id,
            ))
            .clone(),
        };
        Ok(ctx)
    }
}

/// Main Context structure that contains both EvmContext and External context.
/// Serde counterpart of Context struct in revm crate.
#[derive(Serialize, Deserialize)]
#[serde(bound(
    serialize = "DB: Serialize + Database, EXT: Serialize",
    deserialize = "DB: Deserialize<'de> + Database, EXT: Deserialize<'de>"
))]
// #[serde(remote = "Context")]
pub struct ContextSerde<EXT, DB: Database> {
    /// Evm Context.
    #[serde(with = "EvmContextSerde")]
    pub evm: EvmContext<DB>,
    /// External contexts.
    pub external: EXT,
}

impl<EXT, DB: Database> From<ContextSerde<EXT, DB>> for Context<EXT, DB> {
    fn from(ctx: ContextSerde<EXT, DB>) -> Self {
        let evm = ctx.evm;
        let external = ctx.external;
        Context { evm, external }
    }
}

impl<EXT, DB: Database> From<Context<EXT, DB>> for ContextSerde<EXT, DB> {
    fn from(ctx: Context<EXT, DB>) -> Self {
        let evm = ctx.evm;
        let external = ctx.external;
        ContextSerde { evm, external }
    }
}
