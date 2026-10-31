use std::fmt::{self, Display};

use crate::{Observe, ObserveError, Views};

/// Two models judged together: one domain against another (a zone at a block and Tempo at the
/// block it anchors to), or one input run two ways ([`Diff`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pair<A, B> {
    pub left: A,
    pub right: B,
}

/// The same model run two ways that must agree: before and after a fork, two entrypoints given
/// the same arguments, or two states that differ only in data the caller may not see.
pub type Diff<M> = Pair<M, M>;

impl<A: Display, B: Display> Display for Pair<A, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} | {}", self.left, self.right)
    }
}

/// Why a [`Pair`] could not be read: which side failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairError<L, R> {
    Left(ObserveError<L>),
    Right(ObserveError<R>),
}

impl<L: fmt::Display, R: fmt::Display> fmt::Display for PairError<L, R> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Left(e) => write!(f, "left: {e}"),
            Self::Right(e) => write!(f, "right: {e}"),
        }
    }
}

impl<A: Observe, B: Observe> Pair<A, B> {
    /// Reads `A` from `left` and `B` from `right`, two separate states.
    pub fn observe<L: Views, R: Views>(
        left: &mut L,
        at_left: A::At<'_>,
        right: &mut R,
        at_right: B::At<'_>,
    ) -> Result<Self, PairError<L::Error, R::Error>> {
        Ok(Self {
            left: A::observe(left, at_left).map_err(PairError::Left)?,
            right: B::observe(right, at_right).map_err(PairError::Right)?,
        })
    }
}

/// A model built from every step so far rather than read from one state, such as the total ever
/// deposited: what a check compares a state against when the invariant is about history.
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
