use std::fmt::{self, Display};

/// Two values judged together, such as two domains, two runs ([`Diff`]) or a two-part subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pair<A, B> {
    pub left: A,
    pub right: B,
}

impl<A: Display, B: Display> Display for Pair<A, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} | {}", self.left, self.right)
    }
}

/// The same model from two runs that must agree, such as before and after a fork.
pub type Diff<M> = Pair<M, M>;
