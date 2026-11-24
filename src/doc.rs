use std::fmt::{Display, Write as _};

use crate::AnyCheck;

/// The checks as a Markdown table, one row each in the order given.
pub fn markdown<'a, F: Display + 'a>(
    checks: impl IntoIterator<Item = &'a dyn AnyCheck<F>>,
) -> String {
    fn cell(v: Option<impl Display>) -> String {
        v.map(|v| v.to_string().replace('|', "\\|"))
            .unwrap_or_default()
    }
    let mut out = String::from(
        "| ID | Severity | Since | Until | Spec | Covers | Invariant |\n|---|---|---|---|---|---|---|\n",
    );
    for check in checks {
        let meta = check.meta();
        writeln!(
            out,
            "| `{}` | {} | {} | {} | {} | {} | {} |",
            check.id(),
            cell(meta.severity),
            cell(meta.since.as_ref()),
            cell(meta.until.as_ref()),
            cell(meta.tip),
            cell(Some(meta.covers.join(", "))),
            cell(Some(meta.doc)),
        )
        .expect("writing to a string");
    }
    out
}
