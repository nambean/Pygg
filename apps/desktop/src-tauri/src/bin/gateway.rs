//! `gateway`: see `src/cli/gateway.rs`, which `pygg-cli gateway` runs too.

#[tokio::main]
async fn main() {
    smarttaskrouter_desktop_lib::cli::gateway::run(std::env::args().skip(1).collect()).await;
}
