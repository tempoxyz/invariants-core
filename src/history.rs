use std::fmt::{self, Display};

/// A model built from every step so far, such as the total ever deposited.
pub trait Fold<S: ?Sized>: Default {
    fn fold(&mut self, step: &S);
}

/// A state model beside the history folded up to it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folded<H, M> {
    pub history: H,
    pub now: M,
}

impl<H, M: Display> Display for Folded<H, M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.now.fmt(f)
    }
}

/// Folds each step into the history it carries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct History<H> {
    pub folded: H,
    pub steps: u64,
}

impl<H> History<H> {
    /// Folds `step` into the history and counts it.
    pub fn push<S: ?Sized>(&mut self, step: &S)
    where
        H: Fold<S>,
    {
        self.folded.fold(step);
        self.steps += 1;
    }

    /// The history so far beside `now`.
    pub fn at<M>(&self, now: M) -> Folded<H, M>
    where
        H: Clone,
    {
        Folded {
            history: self.folded.clone(),
            now,
        }
    }
}
