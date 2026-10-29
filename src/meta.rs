use std::fmt;

/// What a check asserts and where it applies, kept beside the check so the readable list of
/// invariants (see [`crate::markdown`]) and the gating of a run come from the same place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Meta {
    /// The invariant in one sentence, as a specification states it.
    pub doc: &'static str,
    /// The specification that states it, such as `TIP-1137`.
    pub tip: Option<&'static str>,
    pub severity: Option<Severity>,
    /// The first fork the invariant holds from; every fork when `None`.
    pub since: Option<&'static str>,
    /// The first fork the invariant no longer holds from; none when `None`.
    pub until: Option<&'static str>,
}

impl Meta {
    pub const EMPTY: Self = Self {
        doc: "",
        tip: None,
        severity: None,
        since: None,
        until: None,
    };

    /// Whether the invariant applies at a fork, given the fork's position in the runner's
    /// ordered fork list (`position`, `None` for a fork name the runner does not know).
    pub fn applies(&self, position: impl Fn(&str) -> Option<usize>, current: usize) -> bool {
        let since = self
            .since
            .is_none_or(|fork| position(fork).is_some_and(|p| current >= p));
        let until = self
            .until
            .is_none_or(|fork| position(fork).is_none_or(|p| current < p));
        since && until
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
