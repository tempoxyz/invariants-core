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
            let counts = self.counts.entry(check.id).or_default();
            let violations = &mut self.violations;
            (check.run)(ctx, &mut |subject, verdict| {
                counts.evaluated += 1;
                let Some(verdict) = verdict else { return };
                counts.armed += 1;
                if let Verdict::Broken(detail) = verdict {
                    counts.fired += 1;
                    violations.push(Violation {
                        id: check.id,
                        subject: subject.to_string(),
                        detail,
                    });
                }
            });
        }
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
