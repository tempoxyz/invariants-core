use std::{fmt, ops::Deref, sync::Arc};

use alloy_primitives::{Address, Log, U256};
use alloy_sol_types::{SolCall, SolEvent};

use super::{CallKind, Frame, Outcome};
use crate::{Check, Checked};

/// A storage slot a step changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SlotChange {
    pub address: Address,
    pub slot: U256,
    pub before: U256,
    pub after: U256,
}

/// What a step did, shared by the steps of every model it is judged over.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Effects<T = ()> {
    pub changes: Vec<SlotChange>,
    pub logs: Vec<Log>,
    /// The call frames in entry order, `None` when the runner does not trace calls. Only the EVM's
    /// own calls are frames, not a precompile's internal calls into another.
    pub calls: Option<Vec<Frame>>,
    /// What ran, such as a transaction, a system call or a block.
    pub ran: T,
}

impl<T> Effects<T> {
    /// Whether the step changed any of `address`'s slots.
    pub fn touched(&self, address: Address) -> bool {
        self.changes.iter().any(|change| change.address == address)
    }

    /// The slots the step changed at `address`.
    pub fn changes_at(&self, address: Address) -> impl Iterator<Item = &SlotChange> {
        self.changes
            .iter()
            .filter(move |change| change.address == address)
    }

    /// The logs `address` emitted, in order.
    pub fn logs_from(&self, address: Address) -> impl Iterator<Item = &Log> {
        self.logs.iter().filter(move |log| log.address == address)
    }

    /// The `E` events `address` emitted, in order, skipping logs that do not decode as `E`.
    pub fn events<E: SolEvent>(&self, address: Address) -> Vec<E> {
        self.logs_from(address)
            .filter_map(|log| E::decode_log_data(&log.data).ok())
            .collect()
    }

    /// The logs `frame` and the calls it made emitted.
    pub fn logs_of(&self, frame: &Frame) -> &[Log] {
        self.logs.get(frame.logs.clone()).unwrap_or_default()
    }

    /// The frames that ran `target`'s code on its own storage, none when calls were not traced.
    pub fn calls_to(&self, target: Address) -> impl Iterator<Item = &Frame> {
        self.calls
            .iter()
            .flatten()
            .filter(move |frame| frame.enters(target))
    }

    /// The `C` calls into `target`, with their frames, in order.
    pub fn calls_of<C: SolCall>(&self, target: Address) -> impl Iterator<Item = (&Frame, C)> {
        self.calls_to(target)
            .filter_map(|frame| Some((frame, frame.decode::<C>()?)))
    }

    /// Appends `event` as emitted by `address`.
    pub fn event<E: SolEvent>(mut self, address: Address, event: &E) -> Self {
        self.logs.push(Log {
            address,
            data: event.encode_log_data(),
        });
        self
    }

    /// Appends a top-level `call` from `caller` to `target` that emitted no logs, marking the calls
    /// traced.
    pub fn call<C: SolCall>(
        mut self,
        caller: Address,
        target: Address,
        call: &C,
        outcome: Outcome,
    ) -> Self {
        let logs = self.logs.len();
        self.calls.get_or_insert_default().push(Frame {
            parent: None,
            kind: CallKind::Call,
            caller,
            target,
            input: call.abi_encode().into(),
            value: U256::ZERO,
            committed: outcome.succeeded(),
            outcome,
            logs: logs..logs,
        });
        self
    }
}

/// A model before and after a step, with what the step did. Derefs to its [`Effects`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step<M, T = ()> {
    pub pre: M,
    pub post: M,
    pub effects: Arc<Effects<T>>,
}

impl<M, T> Step<M, T> {
    pub fn new(pre: M, post: M, effects: impl Into<Arc<Effects<T>>>) -> Self {
        Self {
            pre,
            post,
            effects: effects.into(),
        }
    }
}

impl<M, T> Deref for Step<M, T> {
    type Target = Effects<T>;

    fn deref(&self) -> &Effects<T> {
        &self.effects
    }
}

/// A model whose steps have checks: a [`Step`] over it is [`Checked`] against them.
pub trait StepChecked<T: 'static = (), F: 'static = ()>: Sized + 'static {
    const STEP_CHECKS: &'static [Check<Step<Self, T>, F>];
}

impl<M: StepChecked<T, F>, T: 'static, F: 'static> Checked<F> for Step<M, T> {
    const CHECKS: &'static [Check<Self, F>] = M::STEP_CHECKS;
}

/// A model that events change: the step's events replayed onto `pre` give `post`.
pub trait Replay<E>: Sized {
    /// Why the events do not apply, such as a balance that would leave its type.
    type Error: fmt::Display;

    fn replay(&self, events: &[E]) -> Result<Self, Self::Error>;
}
