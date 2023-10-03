use std::fmt::Debug;

use revm::{Database, DatabaseCommit};
use serde::{de::DeserializeOwned, Serialize};

/// The blockchain state on which transactions are executed.
/// BcState extends the Database and DatabaseCommit traits of revm.
/// In addition, BcState should also be serializable for fuzzing usage.
/// BcState should be also cloneable so that multiple transaction can be executed on the same state in parallel during fuzzing. The clone() implementation should be cheap.
pub trait BcState:
    Clone + Debug + Database + DatabaseCommit + Serialize + DeserializeOwned
{
}
