//! Checks reported as assertions to an external runner through a [`Sink`].

mod assertion;
mod signal;

pub use assertion::{
    Assertion, AssertionKind, Condition, Location, Op, Sink, Value, always, sometimes,
};
pub use signal::{Sig, SigKind, SigName};

use crate::{Check, Verdict};

/// Runs `checks` as assertions: `sometimes` armed under `<id>/armed`, `always` held under `<id>`.
pub fn assert<C: ?Sized, F, S: Sink + ?Sized>(checks: &[Check<C, F>], ctx: &C, sink: &mut S) {
    for check in checks {
        let armed_sig = Sig::coverage("{}/armed").fill(&[&check.id]);
        let held_sig = Sig::failure("{}").fill(&[&check.id]);
        (check.run)(ctx, &mut |subject, verdict| {
            sometimes(
                &mut *sink,
                armed_sig.clone(),
                verdict.is_some(),
                subject.to_string(),
            );
            let (holds, message) = match verdict {
                None => return,
                Some(Verdict::Holds) => (true, subject.to_string()),
                Some(Verdict::Broken(detail)) => (false, format!("{subject} {detail}")),
            };
            always(&mut *sink, held_sig.clone(), holds, message);
        });
    }
}
