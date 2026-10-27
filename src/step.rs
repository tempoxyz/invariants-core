use alloy_primitives::{Address, I256, Log, U256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SlotChange {
    pub address: Address,
    pub slot: U256,
    pub before: U256,
    pub after: U256,
}

impl SlotChange {
    pub fn delta(&self) -> Option<I256> {
        let before = I256::try_from(self.before).ok()?;
        let after = I256::try_from(self.after).ok()?;
        after.checked_sub(before)
    }
}

pub fn net_delta<'a>(changes: impl IntoIterator<Item = &'a SlotChange>) -> Option<I256> {
    changes
        .into_iter()
        .try_fold(I256::ZERO, |sum, change| sum.checked_add(change.delta()?))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step<M, T> {
    pub pre: M,
    pub post: M,
    pub changes: Vec<SlotChange>,
    pub logs: Vec<Log>,
    pub tx: T,
}

impl<M, T> Step<M, T> {
    pub fn touched(&self, address: Address) -> bool {
        self.changes.iter().any(|change| change.address == address)
    }

    pub fn changes_at(&self, address: Address) -> impl Iterator<Item = &SlotChange> {
        self.changes
            .iter()
            .filter(move |change| change.address == address)
    }

    pub fn logs_from(&self, address: Address) -> impl Iterator<Item = &Log> {
        self.logs.iter().filter(move |log| log.address == address)
    }

    pub fn with_models<N>(self, pre: N, post: N) -> Step<N, T> {
        Step {
            pre,
            post,
            changes: self.changes,
            logs: self.logs,
            tx: self.tx,
        }
    }
}
