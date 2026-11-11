//! `agent`: see `src/cli/agent.rs`, which `pygg-cli agent` runs too.

#[tokio::main]
async fn main() {
    smarttaskrouter_desktop_lib::cli::agent::run(std::env::args().skip(1).collect()).await;
}
