use std::{fmt, marker::PhantomData};

use alloy_primitives::{Address, Bytes};
use alloy_sol_types::{SolCall, SolInterface, abi::AbiDecoderConfig};

use crate::Pair;

/// A call to run against state.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Call {
    /// The caller, the runner's default when `None`.
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

/// Decodes leniently, so a check never skips an output over an encoding detail. Whether outputs are
/// canonically encoded is a check of its own.
impl Outcome {
    /// What the call returned as `C`'s return value, when it returned and that decodes.
    pub fn returns<C: SolCall>(&self) -> Option<C::Return> {
        let Self::Return(data) = self else {
            return None;
        };
        C::abi_decode_returns(data).ok()
    }

    /// The revert data as one of `E`'s errors, when the call reverted with one. With `E` an
    /// [`alloy_sol_types::ContractError`], `Error(string)` and `Panic(uint256)` decode too.
    pub fn revert<E: SolInterface>(&self) -> Option<E> {
        let Self::Revert(data) = self else {
            return None;
        };
        E::abi_decode(data).ok()
    }

    /// Whether the call returned.
    pub fn succeeded(&self) -> bool {
        matches!(self, Self::Return(_))
    }
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

/// Calls against a state that persist nothing.
pub trait Views {
    type Error;

    /// Runs each of `calls` on the state as is, none seeing another's writes, like `eth_call`s.
    /// `None` answers a call nothing answered: no code, or a key a [`Self::partial`] state lacks.
    fn call(&mut self, calls: &[Call]) -> Result<Vec<Option<Outcome>>, Self::Error>;

    /// Whether the state is partial, so a call may read a key it lacks and go unanswered.
    fn partial(&self) -> bool;
}

/// Calls that run in order on a throwaway copy of a state, so hypothetical execution such as every
/// holder exiting can be read.
pub trait Simulate: Views {
    /// Runs `calls` in order on a throwaway copy of the state, each seeing the writes of those
    /// before it, answered as [`Views::call`] answers.
    fn simulate(&mut self, calls: &[Call]) -> Result<Vec<Option<Outcome>>, Self::Error>;
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

/// A model read from state through [`Views`].
pub trait Observe: Sized {
    /// What to read, such as a token and the accounts whose balances to read.
    type At<'a>: Copy;

    fn observe<V: Views>(views: &mut V, at: Self::At<'_>) -> Result<Self, ObserveError<V::Error>>;
}

/// A model read by running calls on a throwaway copy of the state through [`Simulate`].
pub trait Probe: Sized {
    /// What to run, such as a pool and the holders to exit it.
    type At<'a>: Copy;

    fn probe<V: Simulate>(views: &mut V, at: Self::At<'_>) -> Result<Self, ObserveError<V::Error>>;
}

/// Why a model could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObserveError<E> {
    /// The views themselves failed.
    Views(E),
    /// Nothing answered the call, as [`Views::call`] allows.
    NotAnswered { call: &'static str, to: Address },
    /// The call reverted, halted or returned an undecodable value, or got no answer (`None`).
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
                answer: Some(outcome),
            } => write!(f, "{call} on {to}: {outcome}"),
            Self::Unexpected {
                call,
                to,
                answer: None,
            } => write!(f, "{call} on {to}: no answer"),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for ObserveError<E> {}

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

/// Calls of any types to run in one [`Views::call`], each read back through the [`Handle`] that
/// [`Batch::push`] returned for it.
#[derive(Debug, Default)]
pub struct Batch(Vec<Call>);

impl Batch {
    /// Adds `call` to `to` from `from`, the runner's default caller when `None`.
    pub fn push<C: SolCall>(&mut self, from: Option<Address>, to: Address, call: &C) -> Handle<C> {
        self.0.push(Call {
            from,
            to,
            input: call.abi_encode().into(),
        });
        Handle {
            index: self.0.len() - 1,
            to,
            call: PhantomData,
        }
    }

    /// Runs the calls, each on the state as is.
    pub fn run<V: Views>(&self, views: &mut V) -> Result<Answers, ObserveError<V::Error>> {
        views
            .call(&self.0)
            .map(Answers)
            .map_err(ObserveError::Views)
    }

    /// Runs the calls in order, each seeing the writes of those before it.
    pub fn simulate<V: Simulate>(&self, views: &mut V) -> Result<Answers, ObserveError<V::Error>> {
        views
            .simulate(&self.0)
            .map(Answers)
            .map_err(ObserveError::Views)
    }
}

/// Where a `C` call sits in its [`Batch`].
#[derive(Debug, Clone, Copy)]
pub struct Handle<C> {
    index: usize,
    to: Address,
    call: PhantomData<fn() -> C>,
}

/// The answers to a [`Batch`]'s calls.
#[derive(Debug)]
pub struct Answers(Vec<Option<Outcome>>);

impl Answers {
    /// How `handle`'s call ended, `None` when nothing answered it or the views gave no answer.
    pub fn outcome<C>(&self, handle: &Handle<C>) -> Option<&Outcome> {
        self.0.get(handle.index)?.as_ref()
    }

    /// The value `handle`'s call returned, strictly decoded.
    pub fn decode<C: SolCall, E>(&self, handle: &Handle<C>) -> Result<C::Return, ObserveError<E>> {
        match self.0.get(handle.index) {
            Some(answer) => decode::<C, E>(handle.to, answer.as_ref()),
            None => Err(ObserveError::Unexpected {
                call: C::SIGNATURE,
                to: handle.to,
                answer: None,
            }),
        }
    }
}

/// The value `call` to `to` returned, strictly decoded.
pub fn view<V: Views, C: SolCall>(
    views: &mut V,
    to: Address,
    call: C,
) -> Result<C::Return, ObserveError<V::Error>> {
    let mut batch = Batch::default();
    let handle = batch.push(None, to, &call);
    batch.run(views)?.decode(&handle)
}

/// How `call` to `to` made as `from` ended, `None` when nothing answered it.
pub fn view_from<V: Views, C: SolCall>(
    views: &mut V,
    from: Address,
    to: Address,
    call: C,
) -> Result<Option<Outcome>, ObserveError<V::Error>> {
    let mut batch = Batch::default();
    let handle = batch.push(Some(from), to, &call);
    Ok(batch.run(views)?.outcome(&handle).cloned())
}

/// The values `calls` returned, strictly decoded, `None` for each call nothing answered.
pub fn view_many<V: Views, C: SolCall>(
    views: &mut V,
    calls: &[(Address, C)],
) -> Result<Vec<Option<C::Return>>, ObserveError<V::Error>> {
    let mut batch = Batch::default();
    let handles: Vec<_> = calls
        .iter()
        .map(|(to, call)| batch.push(None, *to, call))
        .collect();
    let answers = batch.run(views)?;
    handles
        .iter()
        .map(|handle| answered(answers.decode(handle)))
        .collect()
}

/// The value a `C` call to `to` returned, strictly decoded from `answer` (`None` if unanswered).
pub fn decode<C: SolCall, E>(
    to: Address,
    answer: Option<&Outcome>,
) -> Result<C::Return, ObserveError<E>> {
    let Some(outcome) = answer else {
        return Err(ObserveError::NotAnswered {
            call: C::SIGNATURE,
            to,
        });
    };
    let value = match outcome {
        Outcome::Return(data) => {
            C::abi_decode_returns_with_config(data, AbiDecoderConfig::new().strict(true)).ok()
        }
        _ => None,
    };
    value.ok_or_else(|| ObserveError::Unexpected {
        call: C::SIGNATURE,
        to,
        answer: Some(outcome.clone()),
    })
}

/// `read`'s value, `None` when nothing answered it.
pub fn answered<T, E>(read: Result<T, ObserveError<E>>) -> Result<Option<T>, ObserveError<E>> {
    match read {
        Ok(value) => Ok(Some(value)),
        Err(err) if err.is_not_answered() => Ok(None),
        Err(err) => Err(err),
    }
}

/// `read`'s value, or `None` if unanswered over [`Views::partial`] views (an error otherwise).
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
