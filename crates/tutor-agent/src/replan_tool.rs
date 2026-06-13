use futures::future::BoxFuture;
use llm_harness_types::{ContentBlock, Tool, ToolContext, ToolError, ToolResult};
use serde_json::json;

static SCHEMA: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();

/// Signals that the current plan should be abandoned and a new plan created.
/// This tool's execute() is never called — ReplanHook intercepts and denies it.
pub struct ReplanTool;

impl Tool for ReplanTool {
    fn name(&self) -> &str {
        "replan"
    }

    fn description(&self) -> &str {
        "Signal that the current plan is insufficient and request a new plan. \
         Provide a specific reason explaining what's wrong and what approach to try instead."
    }

    fn parameters_schema(&self) -> &serde_json::Value {
        SCHEMA.get_or_init(|| {
            json!({
                "type": "object",
                "properties": {
                    "reason": {
                        "type": "string",
                        "description": "Why the current plan failed and what alternative to try"
                    }
                },
                "required": ["reason"]
            })
        })
    }

    fn execute<'a>(
        &'a self,
        _args: serde_json::Value,
        _ctx: &'a ToolContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>> {
        // ReplanHook denies this tool call before it reaches execute().
        Box::pin(async {
            Ok(ToolResult {
                content: vec![ContentBlock::Text {
                    text: "replan acknowledged".into(),
                }],
                details: serde_json::Value::Null,
                terminate: false,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_harness_types::Tool;

    #[test]
    fn replan_tool_has_correct_name_and_schema() {
        let t = ReplanTool;
        assert_eq!(t.name(), "replan");
        let schema = t.parameters_schema();
        assert!(schema["properties"]["reason"].is_object());
        assert_eq!(schema["required"][0], "reason");
    }
}
