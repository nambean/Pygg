//! Phase 0 benchmark CLI. See benchmark/README.md.
//!
//!   bench --cases ../../../benchmark/cases --agent "<command with {prompt_file} {model} {workdir}>" \
//!         --rung T0=<model> --rung T2=<model> --rung T4=<model> [--mode all|single|cascade] [--only <case id>]
//!
//! `single` runs every rung alone (the baselines), `cascade` climbs the rungs on check failure, `all` does both.
//! Results go to one JSON line per case and run in `--out` (default benchmark/runs), and a summary to stdout.

use smarttaskrouter_desktop_lib::bench::{self, CaseRecord, Config, Rung};
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

struct Args {
    cases: PathBuf,
    agent: String,
    rungs: Vec<Rung>,
    mode: String,
    only: Option<String>,
    out: PathBuf,
    agent_timeout: Duration,
    check_timeout: Duration,
}

fn usage() -> ! {
    eprintln!("usage: bench --cases DIR --agent COMMAND --rung TIER=MODEL [--rung …] \
        [--mode all|single|cascade] [--only CASE] [--out DIR] [--agent-timeout SECONDS] [--check-timeout SECONDS]");
    std::process::exit(2);
}

fn parse() -> Args {
    let mut args = std::env::args().skip(1);
    let mut cases = None;
    let mut agent = None;
    let mut rungs = Vec::new();
    let mut mode = "all".to_string();
    let mut only = None;
    let mut out = PathBuf::from("benchmark/runs");
    let mut agent_timeout = Duration::from_secs(900);
    let mut check_timeout = Duration::from_secs(300);
    while let Some(flag) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| usage());
        match flag.as_str() {
            "--cases" => cases = Some(PathBuf::from(value())),
            "--agent" => agent = Some(value()),
            "--rung" => rungs.push(Rung::parse(&value()).unwrap_or_else(|e| { eprintln!("{e}"); usage() })),
            "--mode" => mode = value(),
            "--only" => only = Some(value()),
            "--out" => out = PathBuf::from(value()),
            "--agent-timeout" => agent_timeout = Duration::from_secs(value().parse().unwrap_or_else(|_| usage())),
            "--check-timeout" => check_timeout = Duration::from_secs(value().parse().unwrap_or_else(|_| usage())),
            "-h" | "--help" => usage(),
            other => { eprintln!("unknown argument {other}"); usage() }
        }
    }
    if !matches!(mode.as_str(), "all" | "single" | "cascade") || rungs.is_empty() {
        usage();
    }
    Args {
        cases: cases.unwrap_or_else(|| usage()),
        agent: agent.unwrap_or_else(|| usage()),
        rungs, mode, only, out, agent_timeout, check_timeout,
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = parse();
    let mut cases = bench::load_cases(&args.cases).unwrap_or_else(|e| { eprintln!("{e}"); std::process::exit(1) });
    if let Some(only) = &args.only {
        cases.retain(|c| &c.file.id == only);
    }
    if cases.is_empty() {
        eprintln!("no cases to run");
        std::process::exit(1);
    }
    // Ascending tiers, so a cascade climbs.
    let mut rungs = args.rungs.clone();
    rungs.sort_by_key(|r| r.tier);

    let mut runs: Vec<(Vec<Rung>, bool)> = Vec::new();
    if args.mode != "cascade" {
        runs.extend(rungs.iter().map(|r| (vec![r.clone()], false)));
    }
    if args.mode != "single" && rungs.len() > 1 {
        runs.push((rungs.clone(), true));
    }

    let scratch = std::env::temp_dir().join(format!("str-bench-{}", uuid::Uuid::new_v4().simple()));
    let config = Config { agent: args.agent.clone(), agent_timeout: args.agent_timeout,
        check_timeout: args.check_timeout, scratch: scratch.clone() };
    std::fs::create_dir_all(&args.out).expect("cannot create the output folder");
    let stamp = smarttaskrouter_desktop_lib::db::now_ms();
    let out_path = args.out.join(format!("{stamp}.jsonl"));
    let mut out = std::fs::File::create(&out_path).expect("cannot create the results file");

    let mut records: Vec<CaseRecord> = Vec::new();
    for (run_rungs, cascade) in &runs {
        let label = bench::run_label(run_rungs, *cascade);
        for case in &cases {
            eprint!("{label:<24} {:<32} ", case.file.id);
            match bench::run_case(case, run_rungs, *cascade, &config).await {
                Ok(record) => {
                    eprintln!("{} ({} attempt{}, {:.1}s)", if record.passed { "passed" } else { "FAILED" },
                        record.attempts.len(), if record.attempts.len() == 1 { "" } else { "s" },
                        record.total_ms as f64 / 1000.0);
                    writeln!(out, "{}", serde_json::to_string(&record).unwrap()).expect("cannot write results");
                    records.push(record);
                }
                Err(e) => eprintln!("ERROR {e}"),
            }
        }
    }
    let _ = std::fs::remove_dir_all(&scratch);

    println!("\nRun                      Passed   Attempts   Time");
    for (run_rungs, cascade) in &runs {
        let label = bench::run_label(run_rungs, *cascade);
        let summary = bench::summarize(records.iter().filter(|r| r.run == label));
        println!("{label:<24} {:>3}/{:<4} {:>8}   {:.1}s", summary.passed, summary.cases, summary.attempts,
            summary.total_ms as f64 / 1000.0);
    }
    println!("\nCost: not measured. Agent usage is not metered yet (Phase 1 gateway); every attempt is recorded \
        with cost_status \"unknown\".");
    println!("Results: {}", out_path.display());
}
