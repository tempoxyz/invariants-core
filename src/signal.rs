use std::fmt::{self, Display};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum SigKind {
    Failure,
    Noise,
    HarnessError,
    Setup,
    Coverage,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Sig {
    pub name: &'static str,
    pub kind: SigKind,
}

impl Sig {
    const fn new(name: &'static str, kind: SigKind) -> Self {
        Self { name, kind }
    }

    pub const fn failure(name: &'static str) -> Self {
        Self::new(name, SigKind::Failure)
    }

    pub const fn noise(name: &'static str) -> Self {
        Self::new(name, SigKind::Noise)
    }

    pub const fn harness_error(name: &'static str) -> Self {
        Self::new(name, SigKind::HarnessError)
    }

    pub const fn setup(name: &'static str) -> Self {
        Self::new(name, SigKind::Setup)
    }

    pub const fn coverage(name: &'static str) -> Self {
        Self::new(name, SigKind::Coverage)
    }

    pub fn plain(&self) -> SigName {
        assert!(!self.name.contains("{}"), "a family needs fill");
        SigName {
            name: self.name.to_owned(),
            kind: self.kind,
        }
    }

    pub fn at(&self, instance: impl Display) -> SigName {
        SigName {
            name: format!("{}@{instance}", self.name),
            kind: self.kind,
        }
    }

    pub fn fill(&self, parts: &[&dyn Display]) -> SigName {
        let mut name = String::new();
        let mut rest = self.name;
        let mut parts = parts.iter();
        while let Some((head, tail)) = rest.split_once("{}") {
            name.push_str(head);
            name.push_str(&parts.next().expect("a part per placeholder").to_string());
            rest = tail;
        }
        assert!(parts.next().is_none(), "more parts than placeholders");
        name.push_str(rest);
        SigName {
            name,
            kind: self.kind,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct SigName {
    name: String,
    kind: SigKind,
}

impl SigName {
    pub fn as_str(&self) -> &str {
        &self.name
    }

    pub fn kind(&self) -> SigKind {
        self.kind
    }
}

impl Display for SigName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.name)
    }
}
