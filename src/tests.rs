//! The crate's additions exercised the way a runner uses them.

use alloy_primitives::{Address, Bytes, U256, address};
use alloy_sol_types::{SolCall, SolError, sol};

use crate::{
    AnyCheck, Call, Check, Diff, Effects, FnViews, Fold, History, Meta, ObserveError, Outcome,
    Report, Severity, Step, Verdict, Views, check, ensure, ensure_eq, ensure_le, markdown, view,
    view_from,
};

sol! {
    #[derive(Debug, PartialEq)]
    event AdminSet(address indexed admin);
}

sol! {
    function setAdmin(address admin);
    function balanceOf(address account) returns (uint256);
    interface IPortal {
        error Unauthorized(address caller);
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Fork {
    T1,
    T2,
    T3,
}

impl std::fmt::Display for Fork {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

type PortalStep = Step<Portal, ()>;

/// Only the admin changes the admin: a trace check.
const STEP_CHECKS: &[Check<PortalStep, Fork>] = &[check! {
    id: "TEST-ADMIN-ONLY",
    doc: "Only the current admin can set a new admin.",
    tip: "TIP-0000",
    severity: Severity::High,
    since: Fork::T2,
    when: |s: &PortalStep| s.calls.is_some(),
    over: |s: &PortalStep| [&s.post],
    judge: |s, _| {
        let (frame, _) = s.calls_of::<setAdminCall>(PORTAL).find(|(frame, _)| frame.succeeded())?;
        ensure!(frame.caller == s.pre.admin, "setAdmin by {} while admin is {}", frame.caller, s.pre.admin)
    },
}];

fn set_admin(caller: Address, outcome: Outcome) -> Effects {
    Effects::default().call(caller, PORTAL, &setAdminCall { admin: caller }, outcome)
}

fn step(effects: Effects) -> PortalStep {
    Step::new(Portal { admin: ADMIN }, Portal { admin: ADMIN }, effects)
}

#[test]
fn a_trace_check_judges_who_called_and_skips_untraced_steps() {
    let mut report = Report::default();
    report.register(STEP_CHECKS);
    report.run(STEP_CHECKS, &step(Effects::default()), None);
    let counts = report.counts["TEST-ADMIN-ONLY"];
    assert_eq!((counts.evaluated, counts.armed), (1, 0));
    assert_eq!(
        report.never_armed().collect::<Vec<_>>(),
        ["TEST-ADMIN-ONLY"]
    );

    let returned = Outcome::Return(Bytes::new());
    report.run(STEP_CHECKS, &step(set_admin(ADMIN, returned.clone())), None);
    report.run(
        STEP_CHECKS,
        &step(set_admin(MALLORY, Outcome::Revert(Bytes::new()))),
        None,
    );
    assert!(report.violations.is_empty(), "{:?}", report.violations);
    report.run(STEP_CHECKS, &step(set_admin(MALLORY, returned)), None);
    assert_eq!(report.violations.len(), 1);
    assert_eq!(report.violations[0].id, "TEST-ADMIN-ONLY");
    let counts = report.counts["TEST-ADMIN-ONLY"];
    assert_eq!((counts.evaluated, counts.armed, counts.fired), (4, 2, 1));
}

#[test]
fn metadata_gates_by_fork_and_renders_as_markdown() {
    let meta = STEP_CHECKS[0].meta;
    assert!(!meta.applies(&Fork::T1));
    assert!(meta.applies(&Fork::T2));
    let bounded = Meta {
        until: Some(Fork::T3),
        ..meta
    };
    assert!(!bounded.applies(&Fork::T3));
    assert!(Meta::EMPTY.applies(&Fork::T1));

    let mut report = Report::default();
    report.run(
        STEP_CHECKS,
        &step(set_admin(MALLORY, Outcome::Return(Bytes::new()))),
        Some(&Fork::T1),
    );
    assert!(report.violations.is_empty());
    let counts = report.counts["TEST-ADMIN-ONLY"];
    assert_eq!((counts.evaluated, counts.armed), (1, 0));

    let table = markdown(STEP_CHECKS.iter().map(|c| c as &dyn AnyCheck<Fork>));
    assert!(table.contains("| `TEST-ADMIN-ONLY` | high | T2 |  | TIP-0000 | Only the current admin can set a new admin. |"), "{table}");
}

#[test]
fn comparison_verdicts_name_both_operands() {
    let (supply, cap) = (U256::from(101), U256::from(100));
    assert_eq!(ensure_le!(cap, supply), Some(Verdict::Holds));
    assert_eq!(
        ensure_le!(supply, cap),
        Some(Verdict::Broken("supply <= cap: 101 vs 100".into()))
    );
    assert_eq!(
        ensure_eq!(supply, cap, "token {}", 7),
        Some(Verdict::Broken("token 7: supply == cap: 101 vs 100".into()))
    );
}

#[test]
fn frames_expose_typed_results_and_effects_build_from_typed_events() {
    let returned = setAdminCall { admin: ADMIN };
    let effects = Effects::<()>::default()
        .event(PORTAL, &AdminSet { admin: ADMIN })
        .call(
            ADMIN,
            PORTAL,
            &returned,
            Outcome::Return(balanceOfCall::abi_encode_returns(&U256::from(7)).into()),
        )
        .call(
            MALLORY,
            PORTAL,
            &returned,
            Outcome::Revert(
                IPortal::Unauthorized { caller: MALLORY }
                    .abi_encode()
                    .into(),
            ),
        );
    assert_eq!(
        effects.events::<AdminSet>(PORTAL),
        [AdminSet { admin: ADMIN }]
    );

    let frames = effects.calls.as_deref().unwrap();
    assert_eq!(frames[0].returns::<balanceOfCall>(), Some(U256::from(7)));
    assert!(frames[0].revert::<IPortal::IPortalErrors>().is_none());
    assert!(matches!(
        frames[1].revert::<IPortal::IPortalErrors>(),
        Some(IPortal::IPortalErrors::Unauthorized(e)) if e.caller == MALLORY
    ));
    assert_eq!(frames[1].returns::<balanceOfCall>(), None);
    assert_eq!(
        frames[0].to_string(),
        format!(
            "{ADMIN} -> {PORTAL} 0x{}",
            alloy_primitives::hex::encode(setAdminCall::SELECTOR)
        )
    );

    assert_eq!(Effects::<()>::default().calls_to(PORTAL).count(), 0);
}

#[test]
fn views_tell_unanswered_calls_from_unexpected_ones() {
    let answer = |answer: Option<Outcome>| FnViews {
        call: move |calls: &[Call]| Ok::<_, String>(vec![answer.clone(); calls.len()]),
        partial: true,
    };
    let read = |mut views: FnViews<_>| view(&mut views, PORTAL, balanceOfCall { account: ADMIN });

    let seven = balanceOfCall::abi_encode_returns(&U256::from(7));
    assert_eq!(
        read(answer(Some(Outcome::Return(seven.into())))),
        Ok(U256::from(7))
    );
    assert!(read(answer(None)).unwrap_err().is_not_answered());
    for outcome in [
        Outcome::Return(Bytes::from_static(&[1])),
        Outcome::Revert(Bytes::new()),
        Outcome::Halt,
    ] {
        assert_eq!(
            read(answer(Some(outcome.clone()))),
            Err(ObserveError::Unexpected {
                call: balanceOfCall::SIGNATURE,
                to: PORTAL,
                answer: Some(outcome),
            })
        );
    }
    let mut silent = FnViews {
        call: |_: &[Call]| Ok::<_, String>(vec![]),
        partial: false,
    };
    assert_eq!(
        view(&mut silent, PORTAL, balanceOfCall { account: ADMIN }),
        Err(ObserveError::Unexpected {
            call: balanceOfCall::SIGNATURE,
            to: PORTAL,
            answer: None,
        })
    );
}

/// Answers balanceOf with the balance to its owner only, and reverts for anyone else.
struct Private;

impl Views for Private {
    type Error = String;

    fn call(&mut self, calls: &[Call]) -> Result<Vec<Option<Outcome>>, String> {
        Ok(calls
            .iter()
            .map(|call| {
                let from = call.from?;
                let read = balanceOfCall::abi_decode(&call.input).ok()?;
                Some(if read.account == from {
                    Outcome::Return(balanceOfCall::abi_encode_returns(&U256::from(7)).into())
                } else {
                    Outcome::Revert(Bytes::new())
                })
            })
            .collect())
    }

    fn partial(&self) -> bool {
        false
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
        None,
    );
    assert_eq!(report.violations.len(), 1);
}
