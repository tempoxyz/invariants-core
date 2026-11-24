use std::fmt;

use alloy_primitives::{Address, Bytes, Selector, U256};
use alloy_sol_types::SolCall;

use super::Outcome;

/// How a frame was entered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum CallKind {
    Call,
    StaticCall,
    DelegateCall,
    CallCode,
    Create,
    Create2,
}

/// One call frame of a step, in the order frames were entered.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Frame {
    /// 0 for the transaction's own call.
    pub depth: usize,
    pub kind: CallKind,
    /// `msg.sender` inside the frame.
    pub caller: Address,
    /// The account whose storage the frame runs on, not the code's for a delegate call.
    pub target: Address,
    pub input: Bytes,
    pub value: U256,
    pub outcome: Outcome,
}

impl Frame {
    /// The input as `C`, decoded leniently so it accepts whatever any fork's dispatcher accepted.
    pub fn decode<C: SolCall>(&self) -> Option<C> {
        C::abi_decode(&self.input).ok()
    }

    /// Whether the frame ran `target`'s code with its storage, a plain or static call to it.
    pub fn enters(&self, target: Address) -> bool {
        self.target == target && matches!(self.kind, CallKind::Call | CallKind::StaticCall)
    }
}

/// `caller -> target 0x<selector>`.
impl fmt::Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {}", self.caller, self.target)?;
        match self.input.first_chunk().copied().map(Selector::from) {
            Some(selector) => write!(f, " {selector}"),
            None => Ok(()),
        }
    }
}
