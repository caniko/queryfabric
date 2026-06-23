#![allow(missing_docs)]
//! MCP (Model Context Protocol) integration for build-verification tools.
//!
//! Only available when the `mcp` feature is enabled.

use crate::CmdResult;

/// Format a `CmdResult` into a MCP `CallToolResult`.
pub fn format_result(name: &str, result: CmdResult) -> rmcp::model::CallToolResult {
    let status = if result.success { "PASS" } else { "FAIL" };
    let trunc_note = if result.truncated {
        " (output truncated to last 200 lines)"
    } else {
        ""
    };

    let text = format!(
        "## {name}: {status}  ({:.1}s){trunc_note}\n\n```\n{}\n```",
        result.duration_secs, result.output,
    );

    if result.success {
        rmcp::model::CallToolResult::success(vec![rmcp::model::Content::text(text)])
    } else {
        rmcp::model::CallToolResult::error(vec![rmcp::model::Content::text(text)])
    }
}
