//! `pygg-cli`: one console program for Pygg without its window (src/cli/mod.rs).

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // The MCP server runs its own runtime (src/mcp.rs), so it starts before any other.
    // `pygg mcp setup …` is an ordinary command; only `pygg mcp` itself is the server.
    if args.first().map(String::as_str) == Some("mcp") && args.get(1).map(String::as_str) != Some("setup") {
        pygg_desktop_lib::mcp::run();
        return;
    }
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("runtime");
    let code = runtime.block_on(pygg_desktop_lib::cli::run(args));
    std::process::exit(code);
}
