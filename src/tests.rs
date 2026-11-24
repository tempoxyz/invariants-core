//! Checks, steps and views exercised the way a runner uses them.

use alloy_primitives::{Address, Bytes, U256, address};
use alloy_sol_types::{ContractError, Panic, PanicKind, SolCall, SolError, sol};

use crate::{
    Answers, AnyCheck, Batch, Call, Check, Diff, Effects, FnViews, Fold, Frame, History, Meta,
    ObserveError, Outcome, Report, Severity, Simulate, Step, Verdict, Views, check, ensure,
    ensure_eq, ensure_le, markdown, view, view_from,
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
    covers: ["TEMPO-OLD-1", "TEMPO-OLD-2"],
    severity: Severity::High,
    since: Fork::T2,
    when: |s: &PortalStep| s.calls.is_some(),
    over: |s: &PortalStep| [&s.post],
    judge: |s, _| {
        let (frame, _) = s.calls_of::<setAdminCall>(PORTAL).find(|(frame, _)| frame.outcome.succeeded())?;
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
    report.run(STEP_CHECKS, &step(Effects::default()), None);
    let counts = report.counts["TEST-ADMIN-ONLY"];
    assert_eq!((counts.evaluated, counts.armed), (0, 0));

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
    assert_eq!((counts.evaluated, counts.armed, counts.fired), (3, 2, 1));
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
    assert_eq!((counts.evaluated, counts.armed), (0, 0));

    let table = markdown(STEP_CHECKS.iter().map(|c| c as &dyn AnyCheck<Fork>));
    assert!(table.contains("| `TEST-ADMIN-ONLY` | high | T2 |  | TIP-0000 | TEMPO-OLD-1, TEMPO-OLD-2 | Only the current admin can set a new admin. |"), "{table}");
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
    let call = setAdminCall { admin: ADMIN };
    let mut padded = balanceOfCall::abi_encode_returns(&U256::from(7));
    padded.push(0);
    let effects = Effects::<()>::default()
        .event(PORTAL, &AdminSet { admin: ADMIN })
        .call(
            ADMIN,
            PORTAL,
            &call,
            Outcome::Return(balanceOfCall::abi_encode_returns(&U256::from(7)).into()),
        )
        .call(
            MALLORY,
            PORTAL,
            &call,
            Outcome::Revert(
                IPortal::Unauthorized { caller: MALLORY }
                    .abi_encode()
                    .into(),
            ),
        )
        .call(ADMIN, PORTAL, &call, Outcome::Return(padded.into()))
        .call(
            ADMIN,
            PORTAL,
            &call,
            Outcome::Revert(Panic::from(PanicKind::UnderOverflow).abi_encode().into()),
        );
    assert_eq!(
        effects.events::<AdminSet>(PORTAL),
        [AdminSet { admin: ADMIN }]
    );

    let frames = effects.calls.as_deref().unwrap();
    assert_eq!(
        frames[0].outcome.returns::<balanceOfCall>(),
        Some(U256::from(7))
    );
    assert!(
        frames[0]
            .outcome
            .revert::<IPortal::IPortalErrors>()
            .is_none()
    );
    assert!(matches!(
        frames[1].outcome.revert::<IPortal::IPortalErrors>(),
        Some(IPortal::IPortalErrors::Unauthorized(e)) if e.caller == MALLORY
    ));
    assert_eq!(frames[1].outcome.returns::<balanceOfCall>(), None);
    assert_eq!(
        frames[2].outcome.returns::<balanceOfCall>(),
        Some(U256::from(7))
    );
    type Errors = ContractError<IPortal::IPortalErrors>;
    assert!(matches!(
        frames[1].outcome.revert::<Errors>(),
        Some(ContractError::CustomError(
            IPortal::IPortalErrors::Unauthorized(_)
        ))
    ));
    assert!(matches!(
        frames[3].outcome.revert::<Errors>(),
        Some(ContractError::Panic(panic)) if panic.kind() == Some(PanicKind::UnderOverflow)
    ));
    assert_eq!(
        frames[0].to_string(),
        format!(
            "{ADMIN} -> {PORTAL} 0x{}",
            alloy_primitives::hex::encode(setAdminCall::SELECTOR)
        )
    );

    assert_eq!(Effects::<()>::default().calls_to(PORTAL).count(), 0);

    // A frame commits only when it returns, and owns the logs in its range.
    assert_eq!(
        frames.iter().map(|f| f.committed).collect::<Vec<_>>(),
        [true, false, true, false]
    );
    assert!(effects.logs_of(&frames[0]).is_empty());
    let emitter = Frame {
        logs: 0..1,
        ..frames[0].clone()
    };
    assert_eq!(effects.logs_of(&emitter), &effects.logs[..]);
    let past_the_end = Frame {
        logs: 1..3,
        ..frames[0].clone()
    };
    assert!(effects.logs_of(&past_the_end).is_empty());
}

/// A counter that every call increments and answers with its new value.
struct Counter(u64);

impl Views for Counter {
    type Error = String;

    fn call(&mut self, calls: &[Call]) -> Result<Vec<Option<Outcome>>, String> {
        let next = balanceOfCall::abi_encode_returns(&U256::from(self.0 + 1));
        Ok(vec![Some(Outcome::Return(next.into())); calls.len()])
    }

    fn partial(&self) -> bool {
        false
    }
}

impl Simulate for Counter {
    fn simulate(&mut self, calls: &[Call]) -> Result<Vec<Option<Outcome>>, String> {
        let start = self.0;
        let answers = (1..=calls.len() as u64)
            .map(|n| {
                let value = balanceOfCall::abi_encode_returns(&U256::from(start + n));
                Some(Outcome::Return(value.into()))
            })
            .collect();
        Ok(answers)
    }
}

#[test]
fn a_simulated_batch_sees_earlier_writes_and_a_viewed_one_does_not() {
    let mut batch = Batch::default();
    let read = balanceOfCall { account: ADMIN };
    let first = batch.push(Some(ADMIN), PORTAL, &read);
    let second = batch.push(Some(MALLORY), PORTAL, &read);
    let mut counter = Counter(4);

    let viewed = batch.run(&mut counter).unwrap();
    let simulated = batch.simulate(&mut counter).unwrap();

    let values = |answers: &Answers| {
        [&first, &second].map(|handle| answers.decode::<_, String>(handle).unwrap())
    };
    assert_eq!(values(&viewed), [U256::from(5), U256::from(5)]);
    assert_eq!(values(&simulated), [U256::from(5), U256::from(6)]);
    assert_eq!(counter.0, 4, "nothing persists");
}

#[test]
fn views_tell_unanswered_calls_from_unexpected_ones() {
    let answering = |answer: Option<Outcome>| FnViews {
        call: move |calls: &[Call]| Ok::<_, String>(vec![answer.clone(); calls.len()]),
        partial: true,
    };
    let read = |mut views: FnViews<_>| view(&mut views, PORTAL, balanceOfCall { account: ADMIN });

    let seven = balanceOfCall::abi_encode_returns(&U256::from(7));
    assert_eq!(
        read(answering(Some(Outcome::Return(seven.into())))),
        Ok(U256::from(7))
    );
    assert!(read(answering(None)).unwrap_err().is_not_answered());
    let mut padded = balanceOfCall::abi_encode_returns(&U256::from(7));
    padded.push(0);
    for outcome in [
        Outcome::Return(Bytes::from_static(&[1])),
        Outcome::Return(padded.into()),
        Outcome::Revert(Bytes::new()),
        Outcome::Halt,
    ] {
        assert_eq!(
            read(answering(Some(outcome.clone()))),
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

#[test]
fn a_batch_decodes_each_call_through_its_handle() {
    let mut batch = Batch::default();
    let balance = batch.push(None, PORTAL, &balanceOfCall { account: ADMIN });
    let set = batch.push(None, PORTAL, &setAdminCall { admin: ADMIN });
    let missing = batch.push(None, PORTAL, &balanceOfCall { account: MALLORY });
    let seven = balanceOfCall::abi_encode_returns(&U256::from(7));
    let reverted = Outcome::Revert(Bytes::from_static(&[1]));
    let mut views = FnViews {
        call: |calls: &[Call]| {
            assert_eq!(calls.len(), 3);
            Ok::<_, String>(vec![
                Some(Outcome::Return(seven.clone().into())),
                Some(Outcome::Revert(Bytes::from_static(&[1]))),
            ])
        },
        partial: false,
    };
    let answers = batch.run(&mut views).unwrap();

    assert_eq!(answers.decode::<_, String>(&balance), Ok(U256::from(7)));
    assert_eq!(answers.outcome(&set), Some(&reverted));
    assert_eq!(
        answers.decode::<_, String>(&set).err(),
        Some(ObserveError::Unexpected {
            call: setAdminCall::SIGNATURE,
            to: PORTAL,
            answer: Some(reverted),
        })
    );
    assert_eq!(answers.outcome(&missing), None);
    assert_eq!(
        answers.decode::<_, String>(&missing),
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
    .unwrap()
    .unwrap();
    assert_eq!(own.returns::<balanceOfCall>(), Some(U256::from(7)));
    let other = view_from(
        &mut Private,
        MALLORY,
        PORTAL,
        balanceOfCall { account: ADMIN },
    )
    .unwrap();
    assert!(matches!(other, Some(Outcome::Revert(_))));
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
        doc: "Both runs agree.",
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
