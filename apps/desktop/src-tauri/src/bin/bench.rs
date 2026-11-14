//! Phase 0 benchmark CLI. See benchmark/README.md.
//!
//!   bench --cases ../../../benchmark/cases --agent "<command with {prompt_file} {model} {workdir}>" \
//!         --rung T0=<model> --rung T2=<model> --rung T4=<model> [--mode all|single|cascade] [--only <case id>]
//!
//! `single` runs every rung alone (the baselines), `cascade` climbs the rungs on check failure, `all` does both.
//! Results go to one JSON line per case and run in `--out` (default benchmark/runs), and a summary to stdout.

use pygg_desktop_lib::bench::{self, CaseRecord, Config, Rung};
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
    budget_micro_usd: Option<u64>,
    gateway_db: Option<PathBuf>,
    /// Run each case through the task engine with this agent (ROADMAP N3) instead of `--agent`'s command.
    engine: Option<String>,
    /// Runs of each case per arm, for a pass rate with bounds (ROADMAP N2).
    repeat: usize,
    allow_unknown: bool,
    /// The task engine's handoff after a failed step (ADR-030): none, tail or evidence.
    handoff: Option<String>,
}

fn usage() -> ! {
    eprintln!("usage: bench --cases DIR --agent COMMAND --rung TIER=MODEL [--rung …] \
        [--mode all|single|cascade] [--only CASE] [--out DIR] [--agent-timeout SECONDS] [--check-timeout SECONDS] [--budget-usd N] [--gateway-db STORE] [--engine AGENT] [--repeat N] [--allow-unknown yes] [--handoff none|tail|evidence]");
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
    let mut budget_micro_usd = None;
    let mut gateway_db = None;
    let mut engine = None;
    let mut repeat = 1usize;
    let mut allow_unknown = false;
    let mut handoff = None;
    while let Some(flag) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| usage());
        match flag.as_str() {
            "--cases" => cases = Some(PathBuf::from(value())),
            "--agent" => agent = Some(value()),
            "--rung" => rungs.push(Rung::parse(&value()).unwrap_or_else(|e| {
                eprintln!("{e}");
                usage()
            })),
            "--mode" => mode = value(),
            "--only" => only = Some(value()),
            "--out" => out = PathBuf::from(value()),
            "--agent-timeout" => agent_timeout = Duration::from_secs(value().parse().unwrap_or_else(|_| usage())),
            "--check-timeout" => check_timeout = Duration::from_secs(value().parse().unwrap_or_else(|_| usage())),
            "--budget-usd" => {
                let usd: f64 = value().parse().unwrap_or_else(|_| usage());
                if !(usd.is_finite() && usd > 0.0) {
                    usage()
                }
                budget_micro_usd = Some((usd * 1_000_000.0).round() as u64);
            }
            "--gateway-db" => gateway_db = Some(PathBuf::from(value())),
            "--engine" => engine = Some(value()),
            "--allow-unknown" => allow_unknown = value() == "yes",
            "--handoff" => {
                handoff =
                    Some(value()).filter(|h| ["none", "tail", "evidence"].contains(&h.as_str())).or_else(|| usage())
            }
            "--repeat" => repeat = value().parse().ok().filter(|n| (1..=10).contains(n)).unwrap_or_else(|| usage()),
            "-h" | "--help" => usage(),
            other => {
                eprintln!("unknown argument {other}");
                usage()
            }
        }
    }
    if !matches!(mode.as_str(), "all" | "single" | "cascade") || rungs.is_empty() {
        usage();
    }
    Args {
        cases: cases.unwrap_or_else(|| usage()),
        agent: agent.or_else(|| engine.as_ref().map(|_| String::new())).unwrap_or_else(|| usage()),
        rungs,
        mode,
        only,
        out,
        agent_timeout,
        check_timeout,
        budget_micro_usd,
        gateway_db,
        engine,
        repeat,
        allow_unknown,
        handoff,
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args = parse();
    let mut cases = bench::load_cases(&args.cases).unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(1)
    });
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
    let config = Config {
        agent: args.agent.clone(),
        agent_timeout: args.agent_timeout,
        check_timeout: args.check_timeout,
        scratch: scratch.clone(),
        budget_micro_usd: args.budget_micro_usd,
        gateway_db: args.gateway_db.clone(),
        allow_unknown: args.allow_unknown,
        handoff: args.handoff.clone(),
    };
    let mut ledger = bench::Ledger::default();
    std::fs::create_dir_all(&args.out).expect("cannot create the output folder");
    let stamp = pygg_desktop_lib::db::now_ms();
    let out_path = args.out.join(format!("{stamp}.jsonl"));
    let mut out = std::fs::File::create(&out_path).expect("cannot create the results file");

    let mut records: Vec<CaseRecord> = Vec::new();
    let label_of = |rungs: &[bench::Rung], cascade: bool| match &args.engine {
        Some(_) => format!(
            "engine:{}{}",
            bench::run_label(rungs, cascade),
            args.handoff.as_deref().map(|h| format!(":handoff={h}")).unwrap_or_default()
        ),
        None => bench::run_label(rungs, cascade),
    };
    for (run_rungs, cascade) in &runs {
        let label = label_of(run_rungs, *cascade);
        for case in &cases {
            for round in 1..=args.repeat {
                let tag = if args.repeat > 1 { format!("{} #{round}", case.file.id) } else { case.file.id.clone() };
                eprint!("{label:<52} {tag:<36} ");
                let result = match &args.engine {
                    Some(agent) => bench::run_case_engine(case, run_rungs, *cascade, agent, &config, &mut ledger).await,
                    None => bench::run_case(case, run_rungs, *cascade, &config, &mut ledger).await,
                };
                // Each round gets fresh folders: the engine's store and worktrees are per case and label. BENCH_KEEP_SCRATCH
                // keeps them, to read an attempt's log when a run goes wrong.
                if std::env::var_os("BENCH_KEEP_SCRATCH").is_none() {
                    let _ = std::fs::remove_dir_all(&scratch);
                }
                match result {
                    Ok(mut record) => {
                        // The record carries the arm's whole label, the handoff mode included, so arms stay apart.
                        record.run = label.clone();
                        let cost = record.cost_micro_usd().map(bench::usd).unwrap_or_else(|| "cost unknown".into());
                        let outcome = if record.passed {
                            "passed"
                        } else if record.stopped.is_some() {
                            "STOPPED"
                        } else if record.had_agent_error() {
                            "AGENT ERROR"
                        } else {
                            "FAILED"
                        };
                        eprintln!(
                            "{outcome} ({} attempt{}, {:.1}s, {cost})",
                            record.attempts.len(),
                            if record.attempts.len() == 1 { "" } else { "s" },
                            record.total_ms as f64 / 1000.0
                        );
                        writeln!(out, "{}", serde_json::to_string(&record).unwrap()).expect("cannot write results");
                        records.push(record);
                    }
                    Err(e) => eprintln!("ERROR {e}"),
                }
            }
        }
    }
    if std::env::var_os("BENCH_KEEP_SCRATCH").is_none() {
        let _ = std::fs::remove_dir_all(&scratch);
    } else {
        eprintln!("Kept {}", scratch.display());
    }

    println!("\n{:<52} Passed   Attempts   Time      Est. cost   Per success   Agent errors", "Run");
    for (run_rungs, cascade) in &runs {
        let label = label_of(run_rungs, *cascade);
        let summary = bench::summarize(records.iter().filter(|r| r.run == label));
        let show = |v: Option<u64>| v.map(bench::usd).unwrap_or_else(|| "unknown".into());
        println!(
            "{label:<52} {:>3}/{:<4} {:>8}   {:>7.1}s  {:>10}   {:>11}   {}",
            summary.passed,
            summary.cases,
            summary.attempts,
            summary.total_ms as f64 / 1000.0,
            show(summary.cost_micro_usd),
            show(summary.cost_per_success()),
            summary.agent_errors
        );
    }
    let source = if config.gateway_db.is_some() {
        "Attempts sent through the gateway are costed from its ledger at its prices; others from the agent's own \
         estimate at catalog list price. Neither is a provider bill; a subscription's use is quota, not money"
    } else {
        "Costs are the agent's own estimates at catalog list price, not provider bills"
    };
    println!(
        "\nEstimated total: {}. {source}; \"unknown\" means no cost could be read.",
        bench::usd(ledger.spent_micro_usd)
    );
    if let Some(reason) = ledger.stop_reason(&config) {
        println!("Stopped early: {reason}.");
    }
    println!("Results: {}", out_path.display());
}
