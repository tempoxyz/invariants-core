//! Checks over EVM state: models read through view calls, and the steps that change them.

mod step;
mod trace;
mod views;

pub use step::{Effects, Replay, SlotChange, Step, StepChecked};
pub use trace::{CallKind, Frame};
pub use views::{
    Call, FnViews, Observe, ObserveError, Outcome, PairError, Views, answered, decode, optional,
    view, view_from, view_many,
};
