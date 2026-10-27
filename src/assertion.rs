use std::cmp::Ordering;

use alloy_primitives::{I256, U256};

use crate::signal::SigName;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum Value {
    Unsigned(U256),
    Signed(I256),
}

impl Ord for Value {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Unsigned(a), Self::Unsigned(b)) => a.cmp(b),
            (Self::Signed(a), Self::Signed(b)) => a.cmp(b),
            (Self::Signed(a), Self::Unsigned(b)) => {
                if a.is_negative() {
                    Ordering::Less
                } else {
                    a.into_raw().cmp(b)
                }
            }
            (Self::Unsigned(_), Self::Signed(_)) => other.cmp(self).reverse(),
        }
    }
}

impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

macro_rules! unsigned {
    ($($t:ty),*) => {$(
        impl From<$t> for Value {
            fn from(value: $t) -> Self {
                Self::Unsigned(U256::from(value))
            }
        }
    )*};
}

macro_rules! signed {
    ($($t:ty),*) => {$(
        impl From<$t> for Value {
            fn from(value: $t) -> Self {
                Self::Signed(I256::try_from(i128::from(value)).expect("i128 fits in I256"))
            }
        }
    )*};
}

unsigned!(u8, u16, u32, u64, u128, U256);
signed!(i8, i16, i32, i64, i128);

impl From<usize> for Value {
    fn from(value: usize) -> Self {
        Self::Unsigned(U256::from(value))
    }
}

impl From<isize> for Value {
    fn from(value: isize) -> Self {
        Self::from(value as i64)
    }
}

impl From<I256> for Value {
    fn from(value: I256) -> Self {
        Self::Signed(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum Op {
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum Condition {
    Bool(bool),
    Cmp { op: Op, x: Value, y: Value },
}

impl Condition {
    pub fn cmp(op: Op, x: impl Into<Value>, y: impl Into<Value>) -> Self {
        Self::Cmp {
            op,
            x: x.into(),
            y: y.into(),
        }
    }

    pub fn evaluate(&self) -> bool {
        match *self {
            Self::Bool(holds) => holds,
            Self::Cmp { op, x, y } => match op {
                Op::Lt => x < y,
                Op::Le => x <= y,
                Op::Gt => x > y,
                Op::Ge => x >= y,
                Op::Eq => x == y,
                Op::Ne => x != y,
            },
        }
    }
}

impl From<bool> for Condition {
    fn from(holds: bool) -> Self {
        Self::Bool(holds)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Location {
    pub file: &'static str,
    pub line: u32,
    pub column: u32,
}

impl Location {
    #[track_caller]
    pub fn caller() -> Self {
        let location = std::panic::Location::caller();
        Self {
            file: location.file(),
            line: location.line(),
            column: location.column(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum AssertionKind {
    Always,
    Sometimes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Assertion {
    pub kind: AssertionKind,
    pub signature: SigName,
    pub condition: Condition,
    pub holds: bool,
    pub message: String,
    pub location: Location,
}

pub trait Sink {
    fn record(&mut self, assertion: Assertion);
}

impl Sink for Vec<Assertion> {
    fn record(&mut self, assertion: Assertion) {
        self.push(assertion);
    }
}

impl<S: Sink + ?Sized> Sink for &mut S {
    fn record(&mut self, assertion: Assertion) {
        (**self).record(assertion);
    }
}

#[track_caller]
pub fn always<S: Sink + ?Sized>(
    sink: &mut S,
    signature: SigName,
    condition: impl Into<Condition>,
    message: impl Into<String>,
) -> bool {
    assert_kind(
        sink,
        AssertionKind::Always,
        signature,
        condition.into(),
        message.into(),
    )
}

#[track_caller]
pub fn sometimes<S: Sink + ?Sized>(
    sink: &mut S,
    signature: SigName,
    condition: impl Into<Condition>,
    message: impl Into<String>,
) -> bool {
    assert_kind(
        sink,
        AssertionKind::Sometimes,
        signature,
        condition.into(),
        message.into(),
    )
}

#[track_caller]
fn assert_kind<S: Sink + ?Sized>(
    sink: &mut S,
    kind: AssertionKind,
    signature: SigName,
    condition: Condition,
    message: String,
) -> bool {
    let holds = condition.evaluate();
    sink.record(Assertion {
        kind,
        signature,
        condition,
        holds,
        message,
        location: Location::caller(),
    });
    holds
}
