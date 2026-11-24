mod check;
mod doc;
mod evm;
mod history;
mod meta;
mod pair;
#[cfg(feature = "sink")]
mod sink;

pub use check::{AnyCheck, Check, Checked, Counts, Emit, Report, Verdict, Violation, each};
pub use doc::markdown;
pub use evm::*;
pub use history::{Fold, Folded, History};
pub use meta::{Meta, Severity};
pub use pair::{Diff, Pair};
#[cfg(feature = "sink")]
pub use sink::*;

#[cfg(test)]
mod tests;
