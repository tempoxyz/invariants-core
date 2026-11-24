use std::fmt;

use alloy_primitives::{Address, Bytes, U256};
use alloy_sol_types::{SolCall, SolInterface};

use crate::Outcome;

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
    /// The code's account; for a delegate call, the account whose storage it runs on.
    pub target: Address,
    pub input: Bytes,
    pub value: U256,
    pub outcome: Outcome,
}

impl Frame {
    /// The frame's input as `C`, when its selector is `C`'s and it decodes.
    pub fn decode<C: SolCall>(&self) -> Option<C> {
        C::abi_decode(&self.input).ok()
    }

    /// What the frame returned as `C`'s return value, when it returned and that decodes.
    pub fn returns<C: SolCall>(&self) -> Option<C::Return> {
        match &self.outcome {
            Outcome::Return(data) => C::abi_decode_returns(data).ok(),
            _ => None,
        }
    }

    /// The frame's revert data as one of `E`'s errors, when it reverted with one.
    pub fn revert<E: SolInterface>(&self) -> Option<E> {
        match &self.outcome {
            Outcome::Revert(data) => E::abi_decode(data).ok(),
            _ => None,
        }
    }

    pub fn succeeded(&self) -> bool {
        matches!(self.outcome, Outcome::Return(_))
    }

    /// Whether the frame ran `target`'s code with its storage, a plain or static call to it.
    pub fn enters(&self, target: Address) -> bool {
        self.target == target && matches!(self.kind, CallKind::Call | CallKind::StaticCall)
    }
}

/// `caller -> target 0x<selector>`.
impl fmt::Display for Frame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {} 0x", self.caller, self.target)?;
        self.input
            .iter()
            .take(4)
            .try_for_each(|byte| write!(f, "{byte:02x}"))
    }
}
