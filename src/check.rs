use std::any::Any;
use std::collections::BTreeMap;
use std::fmt::{self, Display};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Holds,
    Broken(String),
}

impl Verdict {
    pub fn ensure(holds: bool, detail: impl FnOnce() -> String) -> Self {
        if holds {
            Self::Holds
        } else {
            Self::Broken(detail())
        }
    }
}

#[macro_export]
macro_rules! ensure {
    ($holds:expr, $($detail:tt)+) => {
        ::core::option::Option::Some($crate::Verdict::ensure($holds, || ::std::format!($($detail)+)))
    };
}

#[macro_export]
macro_rules! check {
    (
        id: $id:expr,
        over: |$ctx:ident : &$ty:ty| $over:expr,
        judge: $judge:expr $(,)?
    ) => {
        $crate::Check::<$ty> {
            id: $id,
            run: |$ctx: &$ty, emit| $crate::each($ctx, $over, $judge, emit),
        }
    };
    (
        id: $id:expr,
        judge: |$ctx:ident : &$ty:ty| $body:expr $(,)?
    ) => {
        $crate::Check::<$ty> {
            id: $id,
            run: |ctx: &$ty, emit| {
                let judge = |$ctx: &$ty| -> ::core::option::Option<$crate::Verdict> { $body };
                emit(ctx, judge(ctx))
            },
        }
    };
}

pub type Emit<'a> = &'a mut dyn FnMut(&dyn Display, Option<Verdict>);

pub struct Check<C: ?Sized> {
    pub id: &'static str,
    pub run: fn(&C, Emit<'_>),
}

impl<C: ?Sized> Clone for Check<C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C: ?Sized> Copy for Check<C> {}

impl<C: ?Sized> fmt::Debug for Check<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Check").field("id", &self.id).finish()
    }
}

/// A [`Check`] of any context type, so checks of different contexts share one list.
pub trait AnyCheck: Sync {
    fn id(&self) -> &'static str;

    /// Runs the check on `ctx` if it is the check's context type; `false` otherwise.
    fn run_any(&self, ctx: &dyn Any, emit: Emit<'_>) -> bool;
}

impl<C: 'static> AnyCheck for Check<C> {
    fn id(&self) -> &'static str {
        self.id
    }

    fn run_any(&self, ctx: &dyn Any, emit: Emit<'_>) -> bool {
        match ctx.downcast_ref::<C>() {
            Some(ctx) => {
                (self.run)(ctx, emit);
                true
            }
            None => false,
        }
    }
}

pub fn each<C: ?Sized, S: Display>(
    ctx: &C,
    subjects: impl IntoIterator<Item = S>,
    judge: impl Fn(&C, &S) -> Option<Verdict>,
    emit: Emit<'_>,
) {
    for subject in subjects {
        let verdict = judge(ctx, &subject);
        emit(&subject, verdict);
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Counts {
    pub evaluated: u64,
    pub armed: u64,
    pub fired: u64,
}

impl Counts {
    pub fn add(&mut self, other: Counts) {
        self.evaluated += other.evaluated;
        self.armed += other.armed;
        self.fired += other.fired;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub id: &'static str,
    pub subject: String,
    pub detail: String,
}

impl Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {} {}", self.id, self.subject, self.detail)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub counts: BTreeMap<&'static str, Counts>,
    pub violations: Vec<Violation>,
}

impl Report {
    pub fn register<C: ?Sized>(&mut self, checks: &[Check<C>]) {
        for check in checks {
            self.counts.entry(check.id).or_default();
        }
    }

    pub fn run<C: ?Sized>(&mut self, checks: &[Check<C>], ctx: &C) {
        for check in checks {
            self.record(check.id, |emit| (check.run)(ctx, emit));
        }
    }

    /// Registers every check in `checks`, whatever its context.
    pub fn register_any<'a>(&mut self, checks: impl IntoIterator<Item = &'a dyn AnyCheck>) {
        for check in checks {
            self.counts.entry(check.id()).or_default();
        }
    }

    /// Runs the checks in `checks` whose context is `ctx`'s type.
    pub fn run_any<'a>(
        &mut self,
        checks: impl IntoIterator<Item = &'a dyn AnyCheck>,
        ctx: &dyn Any,
    ) {
        for check in checks {
            self.record(check.id(), |emit| {
                check.run_any(ctx, emit);
            });
        }
    }

    fn record(&mut self, id: &'static str, run: impl FnOnce(Emit<'_>)) {
        let counts = self.counts.entry(id).or_default();
        let violations = &mut self.violations;
        run(&mut |subject, verdict| {
            counts.evaluated += 1;
            let Some(verdict) = verdict else { return };
            counts.armed += 1;
            if let Verdict::Broken(detail) = verdict {
                counts.fired += 1;
                violations.push(Violation {
                    id,
                    subject: subject.to_string(),
                    detail,
                });
            }
        });
    }

    pub fn merge(&mut self, other: Report) {
        for (id, counts) in other.counts {
            self.counts.entry(id).or_default().add(counts);
        }
        self.violations.extend(other.violations);
    }

    pub fn never_armed(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.counts
            .iter()
            .filter(|(_, counts)| counts.armed == 0)
            .map(|(id, _)| *id)
    }
}

/// Runs `checks` on `ctx` as assertions: per subject, `sometimes` under `<id>/armed` (whether it
/// was armed) and, when armed, `always` under `<id>` (whether it held).
pub fn assert<C: ?Sized, S: crate::Sink + ?Sized>(checks: &[Check<C>], ctx: &C, sink: &mut S) {
    for check in checks {
        let armed_sig = crate::Sig::coverage("{}/armed").fill(&[&check.id]);
        let held_sig = crate::Sig::failure("{}").fill(&[&check.id]);
        (check.run)(ctx, &mut |subject, verdict| {
            crate::sometimes(
                &mut *sink,
                armed_sig.clone(),
                verdict.is_some(),
                subject.to_string(),
            );
            match verdict {
                None => {}
                Some(Verdict::Holds) => {
                    crate::always(&mut *sink, held_sig.clone(), true, subject.to_string());
                }
                Some(Verdict::Broken(detail)) => {
                    crate::always(
                        &mut *sink,
                        held_sig.clone(),
                        false,
                        format!("{subject} {detail}"),
                    );
                }
            }
        });
    }
}

/// A context whose checks are known from its type.
pub trait Checked: Sized + 'static {
    const CHECKS: &'static [Check<Self>];

    /// The violations of [`Self::CHECKS`] in `self`.
    fn check(&self) -> Vec<Violation> {
        let mut report = Report::default();
        report.run(Self::CHECKS, self);
        report.violations
    }
}
