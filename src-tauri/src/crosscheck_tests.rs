//! T-test-001 — Rust side of the shared Rust<->TS crosscheck.
//!
//! Loads the neutral fixture `fixtures/crosscheck-v1.json` (repo root, also read
//! verbatim by `src/crosscheck.test.ts`) and replays each scenario through the
//! *real* production path — `engine::Engine::ingest`, which internally runs
//! `burnrate::compute_pace` / `compute_runway` and the engine's status
//! thresholds — then asserts against `expect.backend`. The frontend loads the
//! same cases and asserts `expect.frontend`, so a lone edit to either end's
//! logic that drifts from the golden fixture turns at least one suite red.
//!
//! Time is relative: the fixture stores `resets_in_secs` and sample offsets as
//! seconds from "now"; this end pins its own fixed fake `now` (`NOW`) and the TS
//! end pins a different one. No absolute timestamps live in the file, so the
//! cases never expire.

#![cfg(test)]

use crate::engine::Engine;
use crate::model::{Limit, LimitStatus, Provider};
use serde_json::Value;
use std::collections::VecDeque;

/// This end's fixed fake wall clock. Arbitrary — pace/runway/status are pure
/// functions of *deltas*, so the absolute value is irrelevant (the TS end uses
/// a different one and still lands on the same golden numbers).
const NOW: i64 = 1_700_000_000;

fn load_fixture() -> Value {
    // Relative to this crate (src-tauri); the fixture lives one level up at the
    // repo root so both language toolchains can reach it. No Cargo.toml change:
    // read at runtime rather than `include_str!`.
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../fixtures/crosscheck-v1.json");
    let raw = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("cannot read crosscheck fixture at {path}: {e}"));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("crosscheck fixture is not valid JSON: {e}"))
}

fn parse_provider(s: &str) -> Provider {
    match s {
        "anthropic" => Provider::Anthropic,
        "codex" => Provider::Codex,
        "grok" => Provider::Grok,
        other => panic!("unknown provider {other:?} in fixture"),
    }
}

fn parse_status(s: &str) -> LimitStatus {
    match s {
        "normal" => LimitStatus::Normal,
        "near" => LimitStatus::Near,
        "locked" => LimitStatus::Locked,
        "stale" => LimitStatus::Stale,
        "insufficient_data" => LimitStatus::InsufficientData,
        "source_failed" => LimitStatus::SourceFailed,
        "idle" => LimitStatus::Idle,
        other => panic!("unknown status {other:?} in fixture"),
    }
}

fn f64_field(v: &Value, key: &str) -> f64 {
    v.get(key)
        .and_then(Value::as_f64)
        .unwrap_or_else(|| panic!("missing/invalid f64 field {key:?}"))
}

fn i64_field(v: &Value, key: &str) -> i64 {
    v.get(key)
        .and_then(Value::as_i64)
        .unwrap_or_else(|| panic!("missing/invalid i64 field {key:?}"))
}

fn str_field<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("missing/invalid str field {key:?}"))
}

/// Build a fresh limit at this end's clock. `resets_in` of 0 keeps `resets_at`
/// at 0 (== unknown, so `compute_pace` returns None) to mirror sources with no
/// reset instant.
fn make_limit(subject: &Value, status: LimitStatus, util: f64) -> Limit {
    let resets_in = i64_field(subject, "resets_in_secs");
    let resets_at = if resets_in > 0 { NOW + resets_in } else { 0 };
    Limit {
        id: str_field(subject, "id").to_string(),
        provider: parse_provider(str_field(subject, "provider")),
        label: str_field(subject, "label").to_string(),
        util,
        resets_at,
        window_secs: i64_field(subject, "window_secs"),
        status,
        absolute: None,
        pace: None,
        runway_secs: None,
        hint: None,
        action: None,
    }
}

/// Replay a scenario through the real engine and return the derived subject.
///
/// Live (incoming `normal`) scenarios replay their sample series as successive
/// ingests — exactly the production data flow — so `runway` is projected from a
/// genuinely-built history and `status`/`pace` come from the same code the app
/// runs. Degraded scenarios carry no samples: a single ingest with the terminal
/// status is preserved by the engine, leaving pace/runway None.
fn run_subject(subject: &Value) -> Limit {
    let id = str_field(subject, "id").to_string();
    let incoming = parse_status(str_field(subject, "status"));
    let mut engine = Engine::new();

    if incoming == LimitStatus::Normal {
        let samples = subject
            .get("samples")
            .and_then(Value::as_array)
            .expect("live case needs a samples array");
        assert!(!samples.is_empty(), "live case {id} has empty samples");
        let mut last: Option<Limit> = None;
        for s in samples {
            let arr = s.as_array().expect("sample must be [t, util]");
            let t = arr[0].as_i64().expect("sample t must be int");
            let u = arr[1].as_f64().expect("sample util must be number");
            let snap = engine.ingest(vec![make_limit(subject, LimitStatus::Normal, u)], NOW + t);
            last = snap.limits.into_iter().find(|l| l.id == id);
        }
        last.expect("subject vanished from snapshot")
    } else {
        let util = f64_field(subject, "util");
        let snap = engine.ingest(vec![make_limit(subject, incoming, util)], NOW);
        snap.limits.into_iter().find(|l| l.id == id).expect("subject vanished")
    }
}

#[test]
fn backend_matches_crosscheck_fixture() {
    let fixture = load_fixture();
    assert_eq!(fixture["version"].as_i64(), Some(1), "unexpected fixture version");
    let cases = fixture["cases"].as_array().expect("fixture.cases must be an array");
    assert!(cases.len() >= 12, "expected >=12 crosscheck cases, got {}", cases.len());

    for case in cases {
        let name = str_field(case, "name");
        let subject = &case["input"]["subject"];
        let expect = &case["expect"]["backend"];

        let derived = run_subject(subject);

        // ── status ──────────────────────────────────────────────────────
        let want_status = str_field(expect, "status");
        let got_status = format!("{:?}", derived.status).to_lowercase();
        // LimitStatus Debug is CamelCase (SourceFailed); compare to snake_case.
        let got_status = match derived.status {
            LimitStatus::SourceFailed => "source_failed".to_string(),
            LimitStatus::InsufficientData => "insufficient_data".to_string(),
            _ => got_status,
        };
        assert_eq!(
            got_status, want_status,
            "[{name}] status: engine derived {got_status:?}, fixture expects {want_status:?}"
        );

        // ── pace ────────────────────────────────────────────────────────
        match expect.get("pace") {
            Some(Value::Null) | None => assert!(
                derived.pace.is_none(),
                "[{name}] pace: expected None, engine produced {:?}",
                derived.pace
            ),
            Some(p) => {
                let pace = derived
                    .pace
                    .unwrap_or_else(|| panic!("[{name}] pace: expected Some, engine produced None"));
                let want_deficit = f64_field(p, "deficit");
                let want_in_deficit =
                    p.get("in_deficit").and_then(Value::as_bool).expect("in_deficit bool");
                assert!(
                    (pace.deficit - want_deficit).abs() <= 0.05,
                    "[{name}] pace.deficit: engine {} vs fixture {} (tol 0.05)",
                    pace.deficit,
                    want_deficit
                );
                assert_eq!(
                    pace.in_deficit, want_in_deficit,
                    "[{name}] pace.in_deficit: engine {} vs fixture {}",
                    pace.in_deficit, want_in_deficit
                );
            }
        }

        // ── runway (±1s tolerance for float truncation) ─────────────────
        match &expect["runway_secs"] {
            Value::Null => assert!(
                derived.runway_secs.is_none(),
                "[{name}] runway: expected None, engine produced {:?}",
                derived.runway_secs
            ),
            want => {
                let want = want.as_i64().expect("runway_secs must be int or null");
                let got = derived
                    .runway_secs
                    .unwrap_or_else(|| panic!("[{name}] runway: expected Some({want}), got None"));
                assert!(
                    (got - want).abs() <= 1,
                    "[{name}] runway: engine {got} vs fixture {want} (tol 1s)"
                );
            }
        }
    }
}

/// Sanity guard on the loader contract itself: the fixture builds valid deques
/// and every live case ends with a sample at t=0. Cheap, and it fails loudly if
/// a future edit breaks the shared file's shape before the assertions above run.
#[test]
fn crosscheck_fixture_shape_is_sane() {
    let fixture = load_fixture();
    for case in fixture["cases"].as_array().unwrap() {
        let subject = &case["input"]["subject"];
        if str_field(subject, "status") == "normal" {
            let samples = subject["samples"].as_array().expect("samples array");
            let last = samples.last().expect("non-empty samples");
            assert_eq!(
                last.as_array().unwrap()[0].as_i64(),
                Some(0),
                "[{}] last sample must sit at t=0",
                str_field(case, "name")
            );
            // Confirm they form a usable history (ascending timestamps).
            let mut dq: VecDeque<i64> = VecDeque::new();
            for s in samples {
                dq.push_back(s.as_array().unwrap()[0].as_i64().unwrap());
            }
            let ascending = dq.iter().zip(dq.iter().skip(1)).all(|(a, b)| a < b);
            assert!(ascending, "sample timestamps must be ascending");
        }
    }
}
