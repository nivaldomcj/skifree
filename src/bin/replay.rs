use std::process::ExitCode;

use skifree::replay::{parse_jsonl, records_to_jsonl, run_scenario, Scenario};

fn usage() -> ! {
    eprintln!("usage: replay <scenario.json> [--update-golden <golden.jsonl>] [--check <golden.jsonl>]");
    std::process::exit(2);
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let scenario_path = match args.next() {
        Some(p) if !p.starts_with('-') => p,
        _ => usage(),
    };

    let mut update_golden: Option<String> = None;
    let mut check: Option<String> = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--update-golden" => update_golden = Some(args.next().unwrap_or_else(|| usage())),
            "--check" => check = Some(args.next().unwrap_or_else(|| usage())),
            _ => usage(),
        }
    }

    let scenario_text = match std::fs::read_to_string(&scenario_path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("replay: cannot read {}: {}", scenario_path, e);
            return ExitCode::FAILURE;
        }
    };
    let scenario: Scenario = match serde_json::from_str(&scenario_text) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("replay: bad scenario {}: {}", scenario_path, e);
            return ExitCode::FAILURE;
        }
    };

    let records = run_scenario(&scenario);
    #[cfg(feature = "rng_sites")]
    skifree::game::rng::rng_sites::dump();
    let jsonl = records_to_jsonl(&records);

    if let Some(ref path) = update_golden {
        if let Err(e) = std::fs::write(&path, &jsonl) {
            eprintln!("replay: cannot write golden {}: {}", path, e);
            return ExitCode::FAILURE;
        }
        eprintln!("replay: wrote {} records to {}", records.len(), path);
    }

    if let Some(ref path) = check {
        let golden_text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("replay: cannot read golden {}: {}", path, e);
                return ExitCode::FAILURE;
            }
        };
        let golden = match parse_jsonl(&golden_text) {
            Ok(g) => g,
            Err(e) => {
                eprintln!("replay: bad golden {}: {}", path, e);
                return ExitCode::FAILURE;
            }
        };
        let divergence = records
            .iter()
            .zip(golden.iter())
            .position(|(a, b)| a != b)
            .or(if records.len() != golden.len() {
                Some(records.len().min(golden.len()))
            } else {
                None
            });
        match divergence {
            None => eprintln!("replay: OK ({} records match)", records.len()),
            Some(tick) => {
                eprintln!("replay: DIVERGED at tick {}", tick);
                return ExitCode::FAILURE;
            }
        }
    }

    if update_golden.is_none() && check.is_none() {
        print!("{}", jsonl);
    } else if let Some(last) = records.last() {
        eprintln!("replay: final hash {:#018x}", last.hash);
    }

    ExitCode::SUCCESS
}
