use std::any::Any;
use std::collections::BTreeMap;
use std::fmt::{self, Display};

use crate::{Meta, Step};

#[macro_export]
macro_rules! ensure {
    ($holds:expr, $($detail:tt)+) => {
        ::core::option::Option::Some($crate::Verdict::ensure($holds, || ::std::format!($($detail)+)))
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __ensure_cmp {
    ($op:tt, $x:expr, $y:expr $(, $($context:tt)+)?) => {
        match (&$x, &$y) {
            (x, y) => ::core::option::Option::Some($crate::Verdict::ensure(x $op y, || {
                let cmp = ::std::format!(
                    "{} {} {}: {:?} vs {:?}",
                    ::core::stringify!($x),
                    ::core::stringify!($op),
                    ::core::stringify!($y),
                    x,
                    y,
                );
                $(let cmp = ::std::format!("{}: {cmp}", ::std::format!($($context)+));)?
                cmp
            })),
        }
    };
}

/// `x == y` as a verdict whose detail names both expressions and their values, after the optional
/// context: `ensure_eq!(supply, sum, "token {token}")` breaks with
/// `token 0x..: supply == sum: 101 vs 100`. The other `ensure_*` macros compare the same way.
#[macro_export]
macro_rules! ensure_eq {
    ($($args:tt)+) => { $crate::__ensure_cmp!(==, $($args)+) };
}

/// [`ensure_eq!`] for `x != y`.
#[macro_export]
macro_rules! ensure_ne {
    ($($args:tt)+) => { $crate::__ensure_cmp!(!=, $($args)+) };
}

/// [`ensure_eq!`] for `x < y`.
#[macro_export]
macro_rules! ensure_lt {
    ($($args:tt)+) => { $crate::__ensure_cmp!(<, $($args)+) };
}

/// [`ensure_eq!`] for `x <= y`.
#[macro_export]
macro_rules! ensure_le {
    ($($args:tt)+) => { $crate::__ensure_cmp!(<=, $($args)+) };
}

/// [`ensure_eq!`] for `x > y`.
#[macro_export]
macro_rules! ensure_gt {
    ($($args:tt)+) => { $crate::__ensure_cmp!(>, $($args)+) };
}

/// [`ensure_eq!`] for `x >= y`.
#[macro_export]
macro_rules! ensure_ge {
    ($($args:tt)+) => { $crate::__ensure_cmp!(>=, $($args)+) };
}

/// A [`Check`]: its ID, optionally its [`Meta`](crate::Meta) (`doc`, `tip`, `severity`, `since`,
/// `until`, in that order) and a `when` guard, then either `over` (the subjects) and `judge`
/// (one verdict per subject), or `judge` alone (one verdict for the context). A context `when`
/// rejects counts as evaluated but not armed, as a `judge` that answers `None` does.
#[macro_export]
macro_rules! check {
    (
        id: $id:expr,
        $(doc: $doc:expr,)?
        $(tip: $tip:expr,)?
        $(severity: $sev:expr,)?
        $(since: $since:expr,)?
        $(until: $until:expr,)?
        $(when: $when:expr,)?
        over: |$ctx:ident : &$ty:ty| $over:expr,
        judge: $judge:expr $(,)?
    ) => {
        $crate::Check::<$ty, _> {
            id: $id,
            meta: $crate::Meta {
                $(doc: $doc,)?
                $(tip: ::core::option::Option::Some($tip),)?
                $(severity: ::core::option::Option::Some($sev),)?
                $(since: ::core::option::Option::Some($since),)?
                $(until: ::core::option::Option::Some($until),)?
                ..$crate::Meta::EMPTY
            },
            run: |$ctx: &$ty, emit| {
                $(
                    let when: fn(&$ty) -> bool = $when;
                    if !when($ctx) {
                        return emit(&$id, ::core::option::Option::None);
                    }
                )?
                $crate::each($ctx, $over, $judge, emit)
            },
        }
    };
    (
        id: $id:expr,
        $(doc: $doc:expr,)?
        $(tip: $tip:expr,)?
        $(severity: $sev:expr,)?
        $(since: $since:expr,)?
        $(until: $until:expr,)?
        $(when: $when:expr,)?
        judge: |$ctx:ident : &$ty:ty| $body:expr $(,)?
    ) => {
        $crate::check! {
            id: $id,
            $(doc: $doc,)?
            $(tip: $tip,)?
            $(severity: $sev,)?
            $(since: $since,)?
            $(until: $until,)?
            $(when: $when,)?
            over: |ctx: &$ty| [ctx],
            judge: |$ctx: &$ty, _: &&$ty| -> ::core::option::Option<$crate::Verdict> { $body },
        }
    };
}

pub type Emit<'a> = &'a mut dyn FnMut(&dyn Display, Option<Verdict>);

pub struct Check<C: ?Sized, F = ()> {
    pub id: &'static str,
    pub meta: Meta<F>,
    pub run: fn(&C, Emit<'_>),
}

impl<C: ?Sized, F: Copy> Clone for Check<C, F> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C: ?Sized, F: Copy> Copy for Check<C, F> {}

impl<C: ?Sized, F> fmt::Debug for Check<C, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Check").field("id", &self.id).finish()
    }
}

impl<C: 'static, F: Sync> AnyCheck<F> for Check<C, F> {
    fn id(&self) -> &'static str {
        self.id
    }

    fn meta(&self) -> &Meta<F> {
        &self.meta
    }

    fn run_any(&self, ctx: &dyn Any, emit: Emit<'_>) {
        if let Some(ctx) = ctx.downcast_ref::<C>() {
            (self.run)(ctx, emit);
        }
    }
}

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

/// A [`Check`] of any context type, so checks of different contexts share one list.
pub trait AnyCheck<F = ()>: Sync {
    fn id(&self) -> &'static str;

    fn meta(&self) -> &Meta<F>;

    /// Runs the check on `ctx` if it is the check's context type.
    fn run_any(&self, ctx: &dyn Any, emit: Emit<'_>);
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
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Report {
    pub counts: BTreeMap<&'static str, Counts>,
    pub violations: Vec<Violation>,
}

impl Report {
    pub fn register<C: ?Sized, F>(&mut self, checks: &[Check<C, F>]) {
        for check in checks {
            self.counts.entry(check.id).or_default();
        }
    }

    /// Runs `checks` on `ctx` at `fork`. A check whose [`Meta`] does not apply at `fork` counts
    /// as evaluated but not armed; without a fork every check applies.
    pub fn run<'a, C: ?Sized + 'a, F: Ord + 'a>(
        &mut self,
        checks: impl IntoIterator<Item = &'a Check<C, F>>,
        ctx: &C,
        fork: Option<&F>,
    ) {
        for check in checks {
            self.record(check.id, &check.meta, fork, |emit| (check.run)(ctx, emit));
        }
    }

    /// Registers every check in `checks`, whatever its context.
    pub fn register_any<'a, F: 'a>(
        &mut self,
        checks: impl IntoIterator<Item = &'a dyn AnyCheck<F>>,
    ) {
        for check in checks {
            self.counts.entry(check.id()).or_default();
        }
    }

    /// Runs the checks in `checks` whose context is `ctx`'s type, at `fork` as [`Self::run`] does.
    pub fn run_any<'a, F: Ord + 'a>(
        &mut self,
        checks: impl IntoIterator<Item = &'a dyn AnyCheck<F>>,
        ctx: &dyn Any,
        fork: Option<&F>,
    ) {
        for check in checks {
            self.record(check.id(), check.meta(), fork, |emit| {
                check.run_any(ctx, emit)
            });
        }
    }

    fn record<F: Ord>(
        &mut self,
        id: &'static str,
        meta: &Meta<F>,
        fork: Option<&F>,
        run: impl FnOnce(Emit<'_>),
    ) {
        let counts = self.counts.entry(id).or_default();
        if fork.is_some_and(|fork| !meta.applies(fork)) {
            counts.evaluated += 1;
            return;
        }
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

/// A context whose checks are known from its type.
pub trait Checked<F: 'static = ()>: Sized + 'static {
    const CHECKS: &'static [Check<Self, F>];

    /// The violations of [`Self::CHECKS`] in `self`, at every fork.
    fn check(&self) -> Vec<Violation>
    where
        F: Ord,
    {
        let mut report = Report::default();
        report.run(Self::CHECKS, self, None);
        report.violations
    }
}

/// A model whose steps have checks: a [`Step`] over it is [`Checked`] against them.
pub trait StepChecked<T: 'static = (), F: 'static = ()>: Sized + 'static {
    const STEP_CHECKS: &'static [Check<Step<Self, T>, F>];
}

impl<M: StepChecked<T, F>, T: 'static, F: 'static> Checked<F> for Step<M, T> {
    const CHECKS: &'static [Check<Self, F>] = M::STEP_CHECKS;
}

pub fn each<C: ?Sized, S: Display>(
    ctx: &C,
    subjects: impl IntoIterator<Item = S>,
    judge: impl Fn(&C, &S) -> Option<Verdict>,
    emit: Emit<'_>,
) {
    for subject in subjects {
        emit(&subject, judge(ctx, &subject));
    }
}

/// Runs `checks` on `ctx` as assertions: per subject, `sometimes` under `<id>/armed` (whether it
/// was armed) and, when armed, `always` under `<id>` (whether it held).
#[cfg(feature = "sink")]
pub fn assert<C: ?Sized, F, S: crate::Sink + ?Sized>(
    checks: &[Check<C, F>],
    ctx: &C,
    sink: &mut S,
) {
    use crate::{Sig, always, sometimes};

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
