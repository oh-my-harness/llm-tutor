use std::sync::{Arc, Mutex};

use futures::future::BoxFuture;
use llm_harness_types::{
    BeforeToolCallCtx, BeforeToolCallDecision, BeforeToolCallHook, ContentBlock, ToolResult,
};
use serde_json::json;

use crate::solve_context::SolveContext;

/// Intercepts `replan()` tool calls, records the reason in SolveContext,
/// and returns Deny so the harness never executes the tool body.
pub struct ReplanHook {
    context: Arc<Mutex<SolveContext>>,
}

impl ReplanHook {
    pub fn new(context: Arc<Mutex<SolveContext>>) -> Self {
        Self { context }
    }
}

impl BeforeToolCallHook for ReplanHook {
    fn on_call<'a>(&'a self, ctx: BeforeToolCallCtx<'a>) -> BoxFuture<'a, BeforeToolCallDecision> {
        Box::pin(async move {
            if ctx.tool_name != "replan" {
                return BeforeToolCallDecision::Allow;
            }
            let reason = ctx.args["reason"].as_str().unwrap_or("").to_string();
            // Lock → write → immediately drop
            self.context.lock().unwrap().replan_reason = Some(reason.clone());
            BeforeToolCallDecision::Deny(ToolResult {
                content: vec![ContentBlock::Text {
                    text: format!("replan triggered: {reason}"),
                }],
                details: json!({ "replan_reason": reason }),
                terminate: false,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_harness_types::{BeforeToolCallCtx, ContentBlock};
    use std::sync::{Arc, Mutex};

    fn make_msg() -> &'static llm_harness_types::AssistantMessage {
        Box::leak(Box::new(llm_harness_types::AssistantMessage {
            content: vec![],
            usage: None,
            stop_reason: None,
            timestamp: chrono::Utc::now(),
            provider: None,
            api: None,
            model: None,
            error_message: None,
        }))
    }

    fn make_ctx<'a>(
        msg: &'a llm_harness_types::AssistantMessage,
        tool_name: &'a str,
        args: &'a serde_json::Value,
    ) -> BeforeToolCallCtx<'a> {
        BeforeToolCallCtx {
            assistant_message: msg,
            tool_use_id: "test-id",
            tool_name,
            args,
            turn_index: 0,
        }
    }

    #[tokio::test]
    async fn non_replan_tool_is_allowed() {
        let ctx = Arc::new(Mutex::new(SolveContext::new("q")));
        let hook = ReplanHook::new(ctx.clone());
        let msg = make_msg();
        let args = serde_json::json!({ "query": "something" });
        let decision = hook.on_call(make_ctx(msg, "rag_search", &args)).await;
        assert!(matches!(decision, BeforeToolCallDecision::Allow));
        assert!(ctx.lock().unwrap().replan_reason.is_none());
    }

    #[tokio::test]
    async fn replan_tool_is_denied_and_reason_written() {
        let ctx = Arc::new(Mutex::new(SolveContext::new("q")));
        let hook = ReplanHook::new(ctx.clone());
        let msg = make_msg();
        let args = serde_json::json!({ "reason": "symbolic math needed" });
        let decision = hook.on_call(make_ctx(msg, "replan", &args)).await;
        match decision {
            BeforeToolCallDecision::Deny(ref result) => match &result.content[0] {
                ContentBlock::Text { text } => assert!(text.contains("replan")),
                _ => panic!("expected text"),
            },
            _ => panic!("expected Deny"),
        }
        assert_eq!(
            ctx.lock().unwrap().replan_reason.as_deref(),
            Some("symbolic math needed")
        );
    }

    #[tokio::test]
    async fn empty_reason_defaults_to_empty_string() {
        let ctx = Arc::new(Mutex::new(SolveContext::new("q")));
        let hook = ReplanHook::new(ctx.clone());
        let msg = make_msg();
        let args = serde_json::json!({});
        let _ = hook.on_call(make_ctx(msg, "replan", &args)).await;
        assert_eq!(ctx.lock().unwrap().replan_reason.as_deref(), Some(""));
    }
}
