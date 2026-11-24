#[cfg(feature = "sink")]
mod assertion;
mod check;
mod doc;
mod meta;
mod pair;
#[cfg(feature = "sink")]
mod signal;
mod step;
mod trace;
mod views;

#[cfg(feature = "sink")]
pub use assertion::{
    Assertion, AssertionKind, Condition, Location, Op, Sink, Value, always, sometimes,
};
#[cfg(feature = "sink")]
pub use check::assert;
pub use check::{
    AnyCheck, Check, Checked, Counts, Emit, Report, StepChecked, Verdict, Violation, each,
};
pub use doc::markdown;
pub use meta::{Meta, Severity};
pub use pair::{Diff, Fold, Folded, History, Pair, PairError};
#[cfg(feature = "sink")]
pub use signal::{Sig, SigKind, SigName};
pub use step::{Effects, Replay, SlotChange, Step, net_delta};
pub use trace::{CallKind, Frame};
pub use views::{
    Call, FnViews, Observe, ObserveError, Outcome, Views, answered, decode, optional, view,
    view_from, view_many,
};

#[cfg(test)]
mod tests;
