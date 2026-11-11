#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // `<app> mcp`: the MCP server for IDE agents on stdin and stdout (src/mcp.rs), without a window. An IDE
    // starts it with pipes, which a windowless program can use.
    if std::env::args().nth(1).as_deref() == Some("mcp") {
        smarttaskrouter_desktop_lib::mcp::run();
        return;
    }
    smarttaskrouter_desktop_lib::run();
}
