//! The crate's additions exercised the way a runner uses them.

use alloy_primitives::{Address, Bytes, U256, address};
use alloy_sol_types::{SolCall, sol};

use crate::{
    AnyCheck, CallKind, Check, Diff, Fold, Frame, History, Meta, Outcome, Report, Severity, Step,
    Views, check, ensure, markdown, view_from,
};

sol! {
    function setAdmin(address admin);
    function balanceOf(address account) returns (uint256);
}

const PORTAL: Address = address!("0x0000000000000000000000000000000000000100");
const ADMIN: Address = address!("0x0000000000000000000000000000000000000a00");
const MALLORY: Address = address!("0x0000000000000000000000000000000000000bad");

#[derive(Debug, Clone, PartialEq, Eq)]
struct Portal {
    admin: Address,
}

impl std::fmt::Display for Portal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "portal")
    }
}

type PortalStep = Step<Portal, ()>;

/// Only the admin changes the admin: a trace check.
const STEP_CHECKS: &[Check<PortalStep>] = &[check! {
    id: "TEST-ADMIN-ONLY",
    doc: "Only the current admin can set a new admin.",
    tip: "TIP-0000",
    severity: Severity::High,
    since: "T2",
    when: |s: &PortalStep| s.traced(),
    over: |s: &PortalStep| [&s.post],
    judge: |s, _| {
        let calls = s.calls_of::<setAdminCall>(PORTAL);
        let (frame, _) = calls.iter().find(|(frame, _)| frame.success)?;
        ensure!(frame.caller == s.pre.admin, "setAdmin by {} while admin is {}", frame.caller, s.pre.admin)
    },
}];

fn frame(caller: Address, success: bool) -> Frame {
    Frame {
        depth: 1,
        kind: CallKind::Call,
        caller,
        target: PORTAL,
        input: setAdminCall { admin: caller }.abi_encode().into(),
        value: U256::ZERO,
        success,
        output: Bytes::new(),
    }
}

fn step(calls: Vec<Frame>) -> PortalStep {
    Step {
        pre: Portal { admin: ADMIN },
        post: Portal { admin: ADMIN },
        changes: vec![],
        logs: vec![],
        calls,
        tx: (),
    }
}

#[test]
fn a_trace_check_judges_who_called() {
    let mut report = Report::default();
    report.run(STEP_CHECKS, &step(vec![frame(ADMIN, true)]));
    report.run(STEP_CHECKS, &step(vec![frame(MALLORY, false)]));
    assert!(report.violations.is_empty(), "{:?}", report.violations);
    report.run(STEP_CHECKS, &step(vec![frame(MALLORY, true)]));
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].id, "TEST-ADMIN-ONLY");
    let counts = report.counts["TEST-ADMIN-ONLY"];
    assert_eq!((counts.evaluated, counts.armed, counts.fired), (3, 2, 1));
}

#[test]
fn an_untraced_step_is_not_evaluated() {
    let mut report = Report::default();
    report.register(STEP_CHECKS);
    report.run(STEP_CHECKS, &step(vec![]));
    assert_eq!(report.counts["TEST-ADMIN-ONLY"].evaluated, 0);
    assert_eq!(
        report.never_armed().collect::<Vec<_>>(),
        ["TEST-ADMIN-ONLY"]
    );
}

#[test]
fn metadata_gates_by_fork_and_renders_as_markdown() {
    let forks = ["T0", "T1", "T2", "T3"];
    let position = |name: &str| forks.iter().position(|f| *f == name);
    let meta = STEP_CHECKS[0].meta;
    assert!(!meta.applies(position, 1));
    assert!(meta.applies(position, 2));
    let bounded = Meta {
        until: Some("T3"),
        ..meta
    };
    assert!(!bounded.applies(position, 3));
    assert!(Meta::EMPTY.applies(position, 0));

    let mut report = Report::default();
    report.run_where(STEP_CHECKS, &step(vec![frame(MALLORY, true)]), |m| {
        m.applies(position, 1)
    });
    assert!(report.violations.is_empty());

    let table = markdown(STEP_CHECKS.iter().map(|c| c as &dyn AnyCheck));
    assert!(table.contains("| \u{60}TEST-ADMIN-ONLY\u{60} | high | T2 |  | TIP-0000 | Only the current admin can set a new admin. |"), "{table}");
}

/// Answers balanceOf with the balance to its owner only, and reverts for anyone else.
struct Private;

impl Views for Private {
    type Error = String;

    fn call(&mut self, calls: &[(Address, Bytes)]) -> Result<Vec<Option<Bytes>>, String> {
        Ok(vec![None; calls.len()])
    }

    fn call_from(
        &mut self,
        from: Address,
        calls: &[(Address, Bytes)],
    ) -> Result<Vec<Option<Outcome>>, String> {
        Ok(calls
            .iter()
            .map(|(_, input)| {
                let call = balanceOfCall::abi_decode(input).ok()?;
                Some(if call.account == from {
                    Outcome::Return(balanceOfCall::abi_encode_returns(&U256::from(7)).into())
                } else {
                    Outcome::Revert(Bytes::new())
                })
            })
            .collect())
    }
}

#[test]
fn a_view_can_be_asked_as_a_third_party() {
    let own = view_from(
        &mut Private,
        ADMIN,
        PORTAL,
        balanceOfCall { account: ADMIN },
    )
    .unwrap();
    assert_eq!(own, Some(Ok(U256::from(7))));
    let other = view_from(
        &mut Private,
        MALLORY,
        PORTAL,
        balanceOfCall { account: ADMIN },
    )
    .unwrap();
    assert!(matches!(other, Some(Err(_))));
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Deposited(u128);

impl Fold<u128> for Deposited {
    fn fold(&mut self, amount: &u128) {
        self.0 += amount;
    }
}

#[test]
fn history_folds_steps_and_pairs_compare_two_runs() {
    let mut history = History::<Deposited>::default();
    for amount in [3u128, 4] {
        history.push(&amount);
    }
    let folded = history.at(Portal { admin: ADMIN });
    assert_eq!((folded.history.0, history.steps), (7, 2));

    const DIFF: &[Check<Diff<Portal>>] = &[check! {
        id: "TEST-SAME",
        judge: |d: &Diff<Portal>| ensure!(d.left == d.right, "{:?} != {:?}", d.left, d.right),
    }];
    let mut report = Report::default();
    report.run(
        DIFF,
        &Diff {
            left: Portal { admin: ADMIN },
            right: Portal { admin: MALLORY },
        },
    );
    assert_eq!(report.violations.len(), 1);
}
