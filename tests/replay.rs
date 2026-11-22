use std::path::Path;

use skifree::replay::{parse_jsonl, records_to_jsonl, run_scenario, Scenario};

fn scenario_path(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("replays")
        .join(format!("{}.json", name))
        .to_string_lossy()
        .into_owned()
}

fn golden_path(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("replays")
        .join("golden")
        .join(format!("{}.jsonl", name))
        .to_string_lossy()
        .into_owned()
}

fn load_scenario(name: &str) -> Scenario {
    let text = std::fs::read_to_string(scenario_path(name))
        .unwrap_or_else(|e| panic!("cannot read {} scenario: {}", name, e));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("bad {} scenario: {}", name, e))
}

fn load_golden(name: &str) -> Vec<skifree::replay::TickRecord> {
    let text = std::fs::read_to_string(golden_path(name))
        .unwrap_or_else(|e| panic!("cannot read {} golden: {}", name, e));
    parse_jsonl(&text).unwrap_or_else(|e| panic!("bad {} golden: {}", name, e))
}

fn check(name: &str) {
    let scenario = load_scenario(name);
    let golden = load_golden(name);
    let records = run_scenario(&scenario);
    if records != golden {
        let divergence = records
            .iter()
            .zip(golden.iter())
            .position(|(a, b)| a != b)
            .unwrap_or_else(|| records.len().min(golden.len()));
        panic!("{} diverges from golden at tick {}", name, divergence);
    }
}

#[test]
fn golden_idle_matches() {
    check("idle");
}

#[test]
fn golden_descend_matches() {
    check("descend");
}

#[test]
fn descend_reaches_finish_area() {
    // Pressing DOWN must actually move the skier downhill (+y).
    let scenario = load_scenario("descend");
    let records = run_scenario(&scenario);
    let first = records.first().unwrap();
    let last = records.last().unwrap();
    let y0 = first.player.as_ref().unwrap().y;
    let y1 = last.player.as_ref().unwrap().y;
    assert!(y1 > y0 + 5000, "skier should descend the course, y {} -> {}", y0, y1);
}

#[test]
fn golden_steer_and_jump_matches() {
    check("steer_and_jump");
}

#[test]
fn actor_pool_never_overflows() {
    let scenario = load_scenario("steer_and_jump");
    let records = run_scenario(&scenario);
    for r in records {
        assert!(r.actor_count <= 100, "actor pool overflow at tick {}", r.tick);
    }
}

#[test]
fn rng_progresses_monotonically() {
    let scenario = load_scenario("steer_and_jump");
    let records = run_scenario(&scenario);
    let mut last = 0u64;
    for r in records {
        assert!(r.rng_calls >= last, "rng call count regressed at tick {}", r.tick);
        last = r.rng_calls;
    }
}

#[test]
fn deterministic_across_runs() {
    let scenario = load_scenario("steer_and_jump");
    let a = run_scenario(&scenario);
    let b = run_scenario(&scenario);
    assert_eq!(a, b);
    let c = records_to_jsonl(&a);
    let d = records_to_jsonl(&b);
    assert_eq!(c, d);
}
