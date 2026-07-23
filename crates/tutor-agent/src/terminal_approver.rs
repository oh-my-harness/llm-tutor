use futures::future::BoxFuture;
use llm_harness_types::{
    BeforeToolCallCtx, BeforeToolCallDecision, BeforeToolCallHook, ToolFailure,
};

/// Reads approval decision from stdin. Only suitable for CLI usage.
pub struct TerminalApprover;

impl BeforeToolCallHook for TerminalApprover {
    fn on_call<'a>(&'a self, ctx: BeforeToolCallCtx<'a>) -> BoxFuture<'a, BeforeToolCallDecision> {
        Box::pin(async move {
            println!("\n[APPROVAL REQUIRED]");
            println!("Tool: {}", ctx.tool_name);
            println!(
                "Args: {}",
                serde_json::to_string_pretty(ctx.args).unwrap_or_default()
            );
            println!("Approve? [y/N]: ");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).ok();
            if input.trim().eq_ignore_ascii_case("y") {
                BeforeToolCallDecision::Allow
            } else {
                BeforeToolCallDecision::Deny(ToolFailure::new(
                    "approval_denied",
                    "Tool execution was denied by the user.",
                ))
            }
        })
    }
}
