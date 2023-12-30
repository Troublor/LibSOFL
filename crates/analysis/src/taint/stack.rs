/// TaintableStack tracks tainted values on the EVM stack.
#[derive(Clone, Debug, Default)]
pub struct TaintableStack {
    stack: Vec<bool>,
}

impl TaintableStack {
    /// Create a new taintable stack.
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }
}

impl TaintableStack {
    pub(crate) fn raw(&self) -> &[bool] {
        &self.stack
    }

    /// Push a number of (un)tainted values to the stack.
    #[deprecated]
    pub(crate) fn push(&mut self, n: usize, tainted: bool) {
        (0..n).into_iter().for_each(|_| self.stack.push(tainted));
    }

    /// Pop a number of values from the stack.
    /// The whether each value is tainted or not is returned as a vector.
    /// The last element of the vector is the top of the stack.
    #[deprecated]
    pub(crate) fn pop(&mut self, n: usize) -> Vec<bool> {
        let mut rs = Vec::new();
        for _ in 0..n {
            let r = self.stack.pop().expect("stack underflow");
            rs.push(r);
        }
        rs.reverse();
        rs
    }

    pub fn taint(&mut self, depth: usize) {
        let l = self.stack.len();
        self.stack[l - depth - 1] = true;
    }

    pub fn clean(&mut self, depth: usize) {
        let l = self.stack.len();
        self.stack[l - depth - 1] = false;
    }

    pub fn is_tainted(&self, depth: usize) -> bool {
        let l = self.stack.len();
        self.stack[l - depth - 1]
    }

    pub fn any_tainted(&self, n: usize) -> bool {
        self.stack.iter().rev().take(n).any(|&t| t)
    }
}

#[allow(unused_macros)]
macro_rules! taint_stack_borrow {
    ($stack:expr, $x1:ident) => {
        let $x1 = $stack.raw().iter().rev().next().expect("stack underflow");
    };
    ($stack:expr, $x1:ident, $x2:ident) => {
        let ($x1, $x2) = {
            let mut iter = $stack.raw().iter().rev();
            (
                iter.next().expect("stack underflow"),
                iter.next().expect("stack underflow"),
            )
        };
    };
    ($stack:expr, $x1:ident, $x2:ident, $x3:ident) => {
        let ($x1, $x2, $x3) = {
            let mut iter = $stack.raw().iter().rev();
            (
                iter.next().expect("stack underflow"),
                iter.next().expect("stack underflow"),
                iter.next().expect("stack underflow"),
            )
        };
    };
    ($stack:expr, $x1:ident, $x2:ident, $x3:ident, $x4:ident) => {
        let ($x1, $x2, $x3, $x4) = {
            let mut iter = $stack.raw().iter().rev();
            (
                iter.next().expect("stack underflow"),
                iter.next().expect("stack underflow"),
                iter.next().expect("stack underflow"),
                iter.next().expect("stack underflow"),
            )
        };
    };
    ($stack:expr, $x1:ident, $x2:ident, $x3:ident, $x4:ident, $x5:ident) => {
        let ($x1, $x2, $x3, $x4, $x5) = {
            let mut iter = $stack.raw().iter().rev();
            (
                iter.next().expect("stack underflow"),
                iter.next().expect("stack underflow"),
                iter.next().expect("stack underflow"),
                iter.next().expect("stack underflow"),
                iter.next().expect("stack underflow"),
            )
        };
    };
    ($stack:expr, $x1:ident, $x2:ident, $x3:ident, $x4:ident, $x5:ident, $x6:ident) => {
        let ($x1, $x2, $x3, $x4, $x5, $x6) = {
            let mut iter = $stack.raw().iter().rev();
            (
                iter.next().expect("stack underflow"),
                iter.next().expect("stack underflow"),
                iter.next().expect("stack underflow"),
                iter.next().expect("stack underflow"),
                iter.next().expect("stack underflow"),
                iter.next().expect("stack underflow"),
            )
        };
    };
    ($stack:expr, $x1:ident, $x2:ident, $x3:ident, $x4:ident, $x5:ident, $x6:ident, $x7:ident) => {
        let ($x1, $x2, $x3, $x4, $x5, $x6, $x7) = {
            let mut iter = $stack.raw().iter().rev();
            (
                iter.next().expect("stack underflow"),
                iter.next().expect("stack underflow"),
                iter.next().expect("stack underflow"),
                iter.next().expect("stack underflow"),
                iter.next().expect("stack underflow"),
                iter.next().expect("stack underflow"),
                iter.next().expect("stack underflow"),
            )
        };
    };
}

#[allow(unused_macros)]
macro_rules! stack_borrow {
    ($interp:expr, $x1:ident) => {
        let mut iter = $interp.stack.data().iter().rev();
        let $x1 = iter.next().expect("stack underflow");
    };
    ($interp:expr, $x1:ident, $x2:ident) => {
        let mut iter = $interp.stack.data().iter().rev();
        let $x1 = iter.next().expect("stack underflow");
        let $x2 = iter.next().expect("stack underflow");
    };
    ($interp:expr, $x1:ident, $x2:ident, $x3:ident) => {
        let mut iter = $interp.stack.data().iter().rev();
        let $x1 = iter.next().expect("stack underflow");
        let $x2 = iter.next().expect("stack underflow");
        let $x3 = iter.next().expect("stack underflow");
    };
    ($interp:expr, $x1:ident, $x2:ident, $x3:ident, $x4:ident) => {
        let mut iter = $interp.stack.data().iter().rev();
        let $x1 = iter.next().expect("stack underflow");
        let $x2 = iter.next().expect("stack underflow");
        let $x3 = iter.next().expect("stack underflow");
        let $x4 = iter.next().expect("stack underflow");
    };
    ($interp:expr, $x1:ident, $x2:ident, $x3:ident, $x4:ident, $x5:ident) => {
        let mut iter = $interp.stack.data().iter().rev();
        let $x1 = iter.next().expect("stack underflow");
        let $x2 = iter.next().expect("stack underflow");
        let $x3 = iter.next().expect("stack underflow");
        let $x4 = iter.next().expect("stack underflow");
        let $x5 = iter.next().expect("stack underflow");
    };
    ($interp:expr, $x1:ident, $x2:ident, $x3:ident, $x4:ident, $x5:ident, $x6:ident) => {
        let mut iter = $interp.stack.data().iter().rev();
        let $x1 = iter.next().expect("stack underflow");
        let $x2 = iter.next().expect("stack underflow");
        let $x3 = iter.next().expect("stack underflow");
        let $x4 = iter.next().expect("stack underflow");
        let $x5 = iter.next().expect("stack underflow");
        let $x6 = iter.next().expect("stack underflow");
    };
    ($interp:expr, $x1:ident, $x2:ident, $x3:ident, $x4:ident, $x5:ident, $x6:ident, $x7:ident) => {
        let mut iter = $interp.stack.data().iter().rev();
        let $x1 = iter.next().expect("stack underflow");
        let $x2 = iter.next().expect("stack underflow");
        let $x3 = iter.next().expect("stack underflow");
        let $x4 = iter.next().expect("stack underflow");
        let $x5 = iter.next().expect("stack underflow");
        let $x6 = iter.next().expect("stack underflow");
        let $x7 = iter.next().expect("stack underflow");
    };
}

macro_rules! stack_delta {
    ($( $op:ident -$consume:literal +$produce:literal )*) => {
        pub const OPCODE_STACK_DELTA: [(usize, usize); 256] = {
            let mut table = [(0, 0); 256];
            $(
                table[libsofl_core::engine::types::opcode::$op as usize] = ($consume as usize, $produce as usize);
            )*
            table
        };
    };
}

// OPCODE_STACK_DELTA
// Mapping from opcode to number of stack elements consumed/produced by the opcode.
stack_delta! {
    ADD -2 +1
    MUL -2 +1
    SUB -2 +1
    DIV -2 +1
    SDIV -2 +1
    MOD -2 +1
    SMOD -2 +1
    ADDMOD -3 +1
    MULMOD -3 +1
    EXP -2 +1
    SIGNEXTEND -2 +1

    LT -2 +1
    GT -2 +1
    SLT -2 +1
    SGT -2 +1
    EQ -2 +1
    ISZERO -1 +1
    AND -2 +1
    OR -2 +1
    XOR -2 +1
    NOT -1 +1
    BYTE -2 +1
    SHL -2 +1
    SHR -2 +1
    SAR -2 +1

    KECCAK256 -2 +1

    ADDRESS -0 +1
    BALANCE -1 +1
    ORIGIN -0 +1
    CALLER -0 +1
    CALLVALUE -0 +1
    CALLDATALOAD -1 +1
    CALLDATASIZE -0 +1
    CALLDATACOPY -3 +0
    CODESIZE -0 +1
    CODECOPY -3 +0

    GASPRICE -0 +1
    EXTCODESIZE -1 +1
    EXTCODECOPY -4 +0
    RETURNDATASIZE -0 +1
    RETURNDATACOPY -3 +0
    EXTCODEHASH -1 +1
    BLOCKHASH -1 +1
    COINBASE -0 +1
    TIMESTAMP -0 +1
    NUMBER -0 +1
    DIFFICULTY -0 +1
    GASLIMIT -0 +1
    CHAINID -0 +1
    SELFBALANCE -0 +1
    BASEFEE -0 +1
    BLOBHASH -1 +1
    BLOBBASEFEE -0 +1

    POP -1 +0
    MLOAD -1 +1
    MSTORE -2 +0
    MSTORE8 -2 +0
    SLOAD -1 +1
    SSTORE -2 +0
    JUMP -1 +0
    JUMPI -2 +0
    PC -0 +1
    MSIZE -0 +1
    GAS -0 +1
    JUMPDEST -0 +0
    TLOAD -1 +1
    TSTORE -2 +0
    MCOPY -3 +0

    PUSH0 -0 +1
    PUSH1 -0 +1
    PUSH2 -0 +1
    PUSH3 -0 +1
    PUSH4 -0 +1
    PUSH5 -0 +1
    PUSH6 -0 +1
    PUSH7 -0 +1
    PUSH8 -0 +1
    PUSH9 -0 +1
    PUSH10 -0 +1
    PUSH11 -0 +1
    PUSH12 -0 +1
    PUSH13 -0 +1
    PUSH14 -0 +1
    PUSH15 -0 +1
    PUSH16 -0 +1
    PUSH17 -0 +1
    PUSH18 -0 +1
    PUSH19 -0 +1
    PUSH20 -0 +1
    PUSH21 -0 +1
    PUSH22 -0 +1
    PUSH23 -0 +1
    PUSH24 -0 +1
    PUSH25 -0 +1
    PUSH26 -0 +1
    PUSH27 -0 +1
    PUSH28 -0 +1
    PUSH29 -0 +1
    PUSH30 -0 +1
    PUSH31 -0 +1
    PUSH32 -0 +1

    DUP1 -1 +2
    DUP2 -2 +3
    DUP3 -3 +4
    DUP4 -4 +5
    DUP5 -5 +6
    DUP6 -6 +7
    DUP7 -7 +8
    DUP8 -8 +9
    DUP9 -9 +10
    DUP10 -10 +11
    DUP11 -11 +12
    DUP12 -12 +13
    DUP13 -13 +14
    DUP14 -14 +15
    DUP15 -15 +16
    DUP16 -16 +17

    SWAP1 -2 +2
    SWAP2 -3 +3
    SWAP3 -4 +4
    SWAP4 -5 +5
    SWAP5 -6 +6
    SWAP6 -7 +7
    SWAP7 -8 +8
    SWAP8 -9 +9
    SWAP9 -10 +10
    SWAP10 -11 +11
    SWAP11 -12 +12
    SWAP12 -13 +13
    SWAP13 -14 +14
    SWAP14 -15 +15
    SWAP15 -16 +16
    SWAP16 -17 +17

    LOG0 -2 +0
    LOG1 -3 +0
    LOG2 -4 +0
    LOG3 -5 +0
    LOG4 -6 +0

    CREATE -3 +1
    CALL -7 +1
    CALLCODE -7 +1
    RETURN -2 +0
    DELEGATECALL -6 +1
    CREATE2 -4 +1

    STATICCALL -6 +1

    REVERT -2 +0
    INVALID -0 +0
    SELFDESTRUCT -1 +0
}
