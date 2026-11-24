use std::fmt;

/// What a check asserts and where it applies, the source of [`crate::markdown`] and fork gating.
/// `F` is the runner's fork type, ordered by activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Meta<F = ()> {
    /// The invariant in one sentence, as a specification states it.
    pub doc: &'static str,
    /// The specification that states it, such as `TIP-1137`.
    pub tip: Option<&'static str>,
    pub severity: Option<Severity>,
    /// The first fork the invariant holds from, every fork when `None`.
    pub since: Option<F>,
    /// The first fork the invariant no longer holds from, `None` if it never stops.
    pub until: Option<F>,
}

impl<F> Meta<F> {
    /// No metadata, applying at every fork.
    pub const EMPTY: Self = Self {
        doc: "",
        tip: None,
        severity: None,
        since: None,
        until: None,
    };

    /// Whether the invariant applies at `fork`.
    pub fn applies(&self, fork: &F) -> bool
    where
        F: Ord,
    {
        self.since.as_ref().is_none_or(|since| fork >= since)
            && self.until.as_ref().is_none_or(|until| fork < until)
    }
}

/// How much a violation costs, the scale of Tempo's specifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum Severity {
    /// Structural inconsistency, operational DoS, accounting drift.
    Medium,
    /// Governance or access-control breakage, privacy breakage, fund lock, replay.
    High,
    /// Direct loss of funds, invalid state transition, unrecoverable corruption.
    Critical,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        })
    }
}
