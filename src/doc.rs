use std::fmt::Write as _;

use crate::AnyCheck;

/// The checks as a Markdown table, one row each in the order given: the readable list of
/// invariants a specification links to, generated so it never drifts from what runs.
pub fn markdown<'a>(checks: impl IntoIterator<Item = &'a dyn AnyCheck>) -> String {
    let mut out = String::from(
        "| ID | Severity | Since | Until | Spec | Invariant |\n|---|---|---|---|---|---|\n",
    );
    for check in checks {
        let meta = check.meta();
        let cell = |v: Option<&str>| v.unwrap_or("").replace('|', "\\|");
        writeln!(
            out,
            "| `{}` | {} | {} | {} | {} | {} |",
            check.id(),
            meta.severity.map(|s| s.to_string()).unwrap_or_default(),
            cell(meta.since),
            cell(meta.until),
            cell(meta.tip),
            meta.doc.replace('|', "\\|"),
        )
        .expect("writing to a string");
    }
    out
}
