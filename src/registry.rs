use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum Layer {
    State,
    Step,
    Block,
    Node,
}

pub struct Check<C: ?Sized> {
    pub id: &'static str,
    pub layer: Layer,
    pub run: fn(&C, &mut Probe),
}

impl<C: ?Sized> Clone for Check<C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C: ?Sized> Copy for Check<C> {}

impl<C: ?Sized> fmt::Debug for Check<C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Check")
            .field("id", &self.id)
            .field("layer", &self.layer)
            .finish()
    }
}

#[derive(Debug)]
pub struct Probe {
    armed: bool,
    failures: Vec<String>,
}

impl Probe {
    fn new() -> Self {
        Self {
            armed: true,
            failures: Vec::new(),
        }
    }

    pub fn when(&mut self, condition: bool) -> bool {
        if !condition {
            self.armed = false;
        }
        condition
    }

    pub fn ensure(&mut self, condition: bool, detail: impl FnOnce() -> String) -> bool {
        if !condition {
            self.failures.push(detail());
        }
        condition
    }

    pub fn fail(&mut self, detail: impl Into<String>) {
        self.failures.push(detail.into());
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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
    pub layer: Layer,
    pub detail: String,
}

impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.id, self.detail)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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
        self.run_where(checks, ctx, |_| true);
    }

    pub fn run_where<C: ?Sized>(
        &mut self,
        checks: &[Check<C>],
        ctx: &C,
        selected: impl Fn(&Check<C>) -> bool,
    ) {
        for check in checks.iter().filter(|check| selected(check)) {
            let mut probe = Probe::new();
            (check.run)(ctx, &mut probe);
            let fired = !probe.failures.is_empty();
            let counts = self.counts.entry(check.id).or_default();
            counts.add(Counts {
                evaluated: 1,
                armed: u64::from(probe.armed || fired),
                fired: u64::from(fired),
            });
            self.violations
                .extend(probe.failures.into_iter().map(|detail| Violation {
                    id: check.id,
                    layer: check.layer,
                    detail,
                }));
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
