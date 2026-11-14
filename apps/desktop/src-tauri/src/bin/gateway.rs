//! `gateway`: see `src/cli/gateway.rs`, which `pygg-cli gateway` runs too.

#[tokio::main]
async fn main() {
    pygg_desktop_lib::cli::gateway::run(std::env::args().skip(1).collect()).await;
}
