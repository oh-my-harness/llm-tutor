use futures::future::BoxFuture;
use llm_harness_runtime::human_approval::{ApprovalDecision, ApprovalRequest, HumanApprover};

/// Reads approval decision from stdin. Only suitable for CLI usage.
pub struct TerminalApprover;

impl HumanApprover for TerminalApprover {
    fn ask<'a>(&'a self, req: &'a ApprovalRequest) -> BoxFuture<'a, Option<ApprovalDecision>> {
        Box::pin(async move {
            println!("\n[APPROVAL REQUIRED]");
            println!("Tool: {}", req.tool_name);
            println!(
                "Args: {}",
                serde_json::to_string_pretty(&req.arguments).unwrap_or_default()
            );
            println!("Approve? [y/N]: ");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).ok();
            if input.trim().eq_ignore_ascii_case("y") {
                Some(ApprovalDecision::Approve)
            } else {
                Some(ApprovalDecision::Deny {
                    reason: "User denied".into(),
                })
            }
        })
    }
}
