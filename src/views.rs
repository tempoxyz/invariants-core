use std::fmt;

use alloy_primitives::{Address, Bytes};
use alloy_sol_types::SolCall;

pub trait Views {
    type Error;

    fn call(&mut self, calls: &[(Address, Bytes)]) -> Result<Vec<Option<Bytes>>, Self::Error>;
}

impl<E, F> Views for F
where
    F: FnMut(&[(Address, Bytes)]) -> Result<Vec<Option<Bytes>>, E>,
{
    type Error = E;

    fn call(&mut self, calls: &[(Address, Bytes)]) -> Result<Vec<Option<Bytes>>, E> {
        self(calls)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObserveError<E> {
    Views(E),
    NotAnswered {
        call: &'static str,
        to: Address,
    },
    Malformed {
        call: &'static str,
        to: Address,
        output: Option<Bytes>,
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
            Self::Malformed {
                call,
                to,
                output: Some(output),
            } => write!(f, "{call} on {to}: malformed output {output}"),
            Self::Malformed {
                call,
                to,
                output: None,
            } => write!(f, "{call} on {to}: no output"),
        }
    }
}

impl<E: fmt::Debug + fmt::Display> std::error::Error for ObserveError<E> {}

pub fn view<V: Views, C: SolCall>(
    views: &mut V,
    to: Address,
    call: C,
) -> Result<C::Return, ObserveError<V::Error>> {
    let outputs = views
        .call(&[(to, call.abi_encode().into())])
        .map_err(ObserveError::Views)?;
    decode::<C, _>(to, outputs.into_iter().next())
}

pub fn view_many<V: Views, C: SolCall>(
    views: &mut V,
    calls: &[(Address, C)],
) -> Result<Vec<Option<C::Return>>, ObserveError<V::Error>> {
    let encoded: Vec<(Address, Bytes)> = calls
        .iter()
        .map(|(to, call)| (*to, call.abi_encode().into()))
        .collect();
    let mut outputs = views
        .call(&encoded)
        .map_err(ObserveError::Views)?
        .into_iter();
    calls
        .iter()
        .map(|(to, _)| match decode::<C, _>(*to, outputs.next()) {
            Ok(value) => Ok(Some(value)),
            Err(err) if err.is_not_answered() => Ok(None),
            Err(err) => Err(err),
        })
        .collect()
}

pub fn decode<C: SolCall, E>(
    to: Address,
    output: Option<Option<Bytes>>,
) -> Result<C::Return, ObserveError<E>> {
    let output = match output {
        Some(Some(output)) => output,
        Some(None) => {
            return Err(ObserveError::NotAnswered {
                call: C::SIGNATURE,
                to,
            });
        }
        None => {
            return Err(ObserveError::Malformed {
                call: C::SIGNATURE,
                to,
                output: None,
            });
        }
    };
    match C::abi_decode_returns(&output) {
        Ok(value) if C::abi_encode_returns(&value) == *output => Ok(value),
        _ => Err(ObserveError::Malformed {
            call: C::SIGNATURE,
            to,
            output: Some(output),
        }),
    }
}

pub struct Complete<F>(pub F);

pub fn complete<F, E>(call: F) -> Complete<F>
where
    F: FnMut(&[(Address, Bytes)]) -> Result<Vec<Bytes>, E>,
{
    Complete(call)
}

impl<E, F> Views for Complete<F>
where
    F: FnMut(&[(Address, Bytes)]) -> Result<Vec<Bytes>, E>,
{
    type Error = E;

    fn call(&mut self, calls: &[(Address, Bytes)]) -> Result<Vec<Option<Bytes>>, E> {
        Ok((self.0)(calls)?.into_iter().map(Some).collect())
    }
}
