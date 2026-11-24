use std::{fmt, ops::Deref, rc::Rc};

use alloy_primitives::{Address, I256, Log, U256};
use alloy_sol_types::{SolCall, SolEvent};

use crate::{CallKind, Frame, Outcome};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SlotChange {
    pub address: Address,
    pub slot: U256,
    pub before: U256,
    pub after: U256,
}

impl SlotChange {
    pub fn delta(&self) -> Option<I256> {
        let before = I256::try_from(self.before).ok()?;
        let after = I256::try_from(self.after).ok()?;
        after.checked_sub(before)
    }
}

pub fn net_delta<'a>(changes: impl IntoIterator<Item = &'a SlotChange>) -> Option<I256> {
    changes
        .into_iter()
        .try_fold(I256::ZERO, |sum, change| sum.checked_add(change.delta()?))
}

/// What a step did, shared by the steps of every model it is judged over.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Effects<T = ()> {
    pub changes: Vec<SlotChange>,
    pub logs: Vec<Log>,
    /// The call frames in the order they were entered; `None` when the runner does not trace
    /// calls.
    pub calls: Option<Vec<Frame>>,
    pub tx: T,
}

impl<T> Effects<T> {
    pub fn touched(&self, address: Address) -> bool {
        self.changes.iter().any(|change| change.address == address)
    }

    pub fn changes_at(&self, address: Address) -> impl Iterator<Item = &SlotChange> {
        self.changes
            .iter()
            .filter(move |change| change.address == address)
    }

    pub fn logs_from(&self, address: Address) -> impl Iterator<Item = &Log> {
        self.logs.iter().filter(move |log| log.address == address)
    }

    /// The `E` events `address` emitted, in order; logs that do not decode as `E` are skipped.
    pub fn events<E: SolEvent>(&self, address: Address) -> Vec<E> {
        self.logs_from(address)
            .filter_map(|log| E::decode_log_data(&log.data).ok())
            .collect()
    }

    /// The frames that ran `target`'s code on its own storage; none when calls were not traced.
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

    /// Appends a top-level call from `caller` to `target`, ending in `outcome`, and marks the
    /// calls traced.
    pub fn call<C: SolCall>(
        mut self,
        caller: Address,
        target: Address,
        call: &C,
        outcome: Outcome,
    ) -> Self {
        self.calls.get_or_insert_default().push(Frame {
            depth: 0,
            kind: CallKind::Call,
            caller,
            target,
            input: call.abi_encode().into(),
            value: U256::ZERO,
            outcome,
        });
        self
    }
}

/// A model before and after a step, with what the step did. Derefs to its [`Effects`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step<M, T = ()> {
    pub pre: M,
    pub post: M,
    pub effects: Rc<Effects<T>>,
}

impl<M, T> Step<M, T> {
    pub fn new(pre: M, post: M, effects: impl Into<Rc<Effects<T>>>) -> Self {
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

/// A model that events change: the model before a step with the step's events replayed onto it
/// is the model after the step.
pub trait Replay<E>: Sized {
    /// Why the events do not apply, such as a balance that would leave its type.
    type Error: fmt::Display;

    fn replay(&self, events: &[E]) -> Result<Self, Self::Error>;
}
