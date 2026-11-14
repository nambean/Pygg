//! `pygg-mcp`: the MCP server for IDE agents (src/mcp.rs), as its own program. The app serves the same with
//! `<Pygg app> mcp`.

fn main() {
    pygg_desktop_lib::mcp::run();
}
