use alloy_primitives::{Address, Bytes, U256};
use alloy_sol_types::SolCall;

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
    /// Whether the frame returned rather than reverted or halted.
    pub success: bool,
    /// What the frame returned, or its revert data.
    pub output: Bytes,
}

impl Frame {
    pub fn selector(&self) -> Option<[u8; 4]> {
        self.input
            .get(..4)
            .map(|s| s.try_into().expect("four bytes"))
    }

    /// The frame's input as `C`, when its selector is `C`'s and it decodes.
    pub fn decode<C: SolCall>(&self) -> Option<C> {
        if self.selector()? != C::SELECTOR {
            return None;
        }
        C::abi_decode(&self.input).ok()
    }

    /// Whether the frame ran `target`'s code with its storage, a plain or static call to it.
    pub fn enters(&self, target: Address) -> bool {
        self.target == target && matches!(self.kind, CallKind::Call | CallKind::StaticCall)
    }
}
