use std::fmt;

use alloy_primitives::{Address, Bytes};
use alloy_sol_types::SolCall;

/// A call to run against state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    /// The caller; the runner's default when `None`.
    pub from: Option<Address>,
    pub to: Address,
    pub input: Bytes,
}

impl Call {
    /// A call to `to` from the runner's default caller.
    pub fn new(to: Address, input: impl Into<Bytes>) -> Self {
        Self {
            from: None,
            to,
            input: input.into(),
        }
    }
}

/// How a call ended.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum Outcome {
    Return(Bytes),
    Revert(Bytes),
    Halt,
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Return(data) => write!(f, "returned {data}"),
            Self::Revert(data) => write!(f, "reverted {data}"),
            Self::Halt => f.write_str("halted"),
        }
    }
}

pub trait Views {
    type Error;

    /// Runs `calls` in order on a scratch copy of the state, each seeing what those before it
    /// changed; nothing persists. One answer per call, `None` when nothing answered it: its target
    /// has no code, or it read a key a [`Self::partial`] state lacks.
    fn call(&mut self, calls: &[Call]) -> Result<Vec<Option<Outcome>>, Self::Error>;

    /// Whether the state is partial, so a call may read a key it lacks and go unanswered.
    fn partial(&self) -> bool;
}

/// [`Views`] answered by a closure.
pub struct FnViews<F> {
    pub call: F,
    pub partial: bool,
}

impl<E, F> Views for FnViews<F>
where
    F: FnMut(&[Call]) -> Result<Vec<Option<Outcome>>, E>,
{
    type Error = E;

    fn call(&mut self, calls: &[Call]) -> Result<Vec<Option<Outcome>>, E> {
        (self.call)(calls)
    }

    fn partial(&self) -> bool {
        self.partial
    }
}

/// `call` to `to` made as `from`: `Ok(value)` when it returned and decoded, `Err(revert data)`
/// when it reverted; `None` when nothing answered it.
pub fn view_from<V: Views, C: SolCall>(
    views: &mut V,
    from: Address,
    to: Address,
    call: C,
) -> Result<Option<Result<C::Return, Bytes>>, ObserveError<V::Error>> {
    let call = Call {
        from: Some(from),
        to,
        input: call.abi_encode().into(),
    };
    match views
        .call(&[call])
        .map_err(ObserveError::Views)?
        .into_iter()
        .next()
    {
        Some(None) => Ok(None),
        Some(Some(Outcome::Revert(data))) => Ok(Some(Err(data))),
        answer => decode::<C, _>(to, answer).map(|v| Some(Ok(v))),
    }
}

/// A model read from state through [`Views`].
pub trait Observe: Sized {
    /// What to read, such as a token and the accounts whose balances to read.
    type At<'a>: Copy;

    fn observe<V: Views>(views: &mut V, at: Self::At<'_>) -> Result<Self, ObserveError<V::Error>>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObserveError<E> {
    Views(E),
    NotAnswered {
        call: &'static str,
        to: Address,
    },
    /// The call reverted, halted or returned what does not decode, or the views gave it no answer
    /// at all (`None`).
    Unexpected {
        call: &'static str,
        to: Address,
        answer: Option<Outcome>,
    },
}

impl<E> ObserveError<E> {
    pub fn is_not_answered(&self) -> bool {
        matches!(self, Self::NotAnswered { .. })
    }
}

impl<E: fmt::Display> fmt::Display for ObserveError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Views(err) => write!(f, "views failed: {err}"),
            Self::NotAnswered { call, to } => write!(f, "{call} on {to}: not answered"),
            Self::Unexpected {
                call,
                to,
                answer: Some(answer),
            } => write!(f, "{call} on {to}: {answer}"),
            Self::Unexpected {
                call,
                to,
                answer: None,
            } => write!(f, "{call} on {to}: no answer"),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for ObserveError<E> {}

pub fn view<V: Views, C: SolCall>(
    views: &mut V,
    to: Address,
    call: C,
) -> Result<C::Return, ObserveError<V::Error>> {
    let answers = views
        .call(&[Call::new(to, call.abi_encode())])
        .map_err(ObserveError::Views)?;
    decode::<C, _>(to, answers.into_iter().next())
}

pub fn view_many<V: Views, C: SolCall>(
    views: &mut V,
    calls: &[(Address, C)],
) -> Result<Vec<Option<C::Return>>, ObserveError<V::Error>> {
    let encoded: Vec<Call> = calls
        .iter()
        .map(|(to, call)| Call::new(*to, call.abi_encode()))
        .collect();
    let mut answers = views
        .call(&encoded)
        .map_err(ObserveError::Views)?
        .into_iter();
    calls
        .iter()
        .map(|(to, _)| answered(decode::<C, _>(*to, answers.next())))
        .collect()
}

/// The value a `C` call to `to` returned, given its answer (`None` when the views gave none).
pub fn decode<C: SolCall, E>(
    to: Address,
    answer: Option<Option<Outcome>>,
) -> Result<C::Return, ObserveError<E>> {
    let unexpected = |answer| ObserveError::Unexpected {
        call: C::SIGNATURE,
        to,
        answer,
    };
    match answer {
        Some(Some(Outcome::Return(output))) => match C::abi_decode_returns(&output) {
            Ok(value) if C::abi_encode_returns(&value) == *output => Ok(value),
            _ => Err(unexpected(Some(Outcome::Return(output)))),
        },
        Some(None) => Err(ObserveError::NotAnswered {
            call: C::SIGNATURE,
            to,
        }),
        answer => Err(unexpected(answer.flatten())),
    }
}

/// `read`'s value, `None` when nothing answered it.
pub fn answered<T, E>(read: Result<T, ObserveError<E>>) -> Result<Option<T>, ObserveError<E>> {
    match read {
        Ok(value) => Ok(Some(value)),
        Err(err) if err.is_not_answered() => Ok(None),
        Err(err) => Err(err),
    }
}

/// `read`'s value. Over [`Views::partial`] views, `None` when nothing answered it; otherwise
/// that is an error.
pub fn optional<V: Views + ?Sized, T>(
    views: &V,
    read: Result<T, ObserveError<V::Error>>,
) -> Result<Option<T>, ObserveError<V::Error>> {
    if views.partial() {
        answered(read)
    } else {
        read.map(Some)
    }
}
