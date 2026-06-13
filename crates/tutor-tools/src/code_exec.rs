use std::time::Duration;

use futures::future::BoxFuture;
use llm_harness_runtime_sandbox_os::OsEnv;
use llm_harness_types::{
    ContentBlock, EnvError, ExecutionEnv, ShellOptions, Tool, ToolContext, ToolError, ToolResult,
};
use serde_json::json;
use tokio_util::sync::CancellationToken;

static SCHEMA: std::sync::OnceLock<serde_json::Value> = std::sync::OnceLock::new();

/// Execute user-supplied code in a temporary directory via OsEnv.
/// v0.1: uses OsEnvSandbox (no real isolation). v0.2 will add bwrap/seatbelt.
pub struct CodeExecTool {
    timeout: Duration,
}

impl CodeExecTool {
    pub fn new() -> Self {
        Self {
            timeout: Duration::from_secs(30),
        }
    }
}

impl Default for CodeExecTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for CodeExecTool {
    fn name(&self) -> &str {
        "code_exec"
    }

    fn description(&self) -> &str {
        "Execute code in a sandboxed environment and return stdout/stderr. \
         Supports 'python', 'bash'. Returns exit code and output."
    }

    fn parameters_schema(&self) -> &serde_json::Value {
        SCHEMA.get_or_init(|| {
            json!({
                "type": "object",
                "properties": {
                    "language": {
                        "type": "string",
                        "enum": ["python", "bash"],
                        "description": "Programming language"
                    },
                    "code": {
                        "type": "string",
                        "description": "Code to execute"
                    }
                },
                "required": ["language", "code"]
            })
        })
    }

    fn execute<'a>(
        &'a self,
        args: serde_json::Value,
        ctx: &'a ToolContext,
    ) -> BoxFuture<'a, Result<ToolResult, ToolError>> {
        Box::pin(async move {
            let language = args["language"].as_str().unwrap_or("bash");
            let code = args["code"]
                .as_str()
                .ok_or_else(|| ToolError::InvalidArguments("missing code".into()))?;

            // Create a temp working directory
            let work_dir = ctx
                .env
                .create_temp_dir("tutor_code_exec")
                .await
                .map_err(|e| ToolError::Execution(e.to_string()))?;

            let env = OsEnv::new(&work_dir);

            // Write source file
            let (filename, run_cmd) = match language {
                "python" => ("script.py", "python3 script.py".to_string()),
                "bash" => ("script.sh", "bash script.sh".to_string()),
                other => {
                    return Err(ToolError::InvalidArguments(format!(
                        "unsupported language: {other}"
                    )));
                }
            };

            env.write_file(
                std::path::Path::new(filename),
                code.as_bytes(),
                CancellationToken::new(),
            )
            .await
            .map_err(|e| ToolError::Execution(e.to_string()))?;

            // Execute — ShellFailed means non-zero exit, NOT a system error
            let opts = ShellOptions {
                cwd: Some(&work_dir),
                timeout: Some(self.timeout),
                abort: ctx.abort.clone(),
                env: vec![],
                on_stdout: None,
                on_stderr: None,
            };

            let (stdout, stderr, exit_code) = match env.execute_shell(&run_cmd, opts).await {
                Ok(out) => (out.stdout, out.stderr, out.exit_code),
                Err(EnvError::ShellFailed { exit_code, stderr }) => {
                    (String::new(), stderr, exit_code)
                }
                Err(e) => return Err(ToolError::Execution(e.to_string())),
            };

            let output = format!(
                "exit_code: {exit_code}\n\
                 stdout:\n{stdout}\
                 {}",
                if stderr.is_empty() {
                    String::new()
                } else {
                    format!("stderr:\n{stderr}")
                }
            );

            Ok(ToolResult {
                content: vec![ContentBlock::Text { text: output }],
                details: json!({
                    "language": language,
                    "exit_code": exit_code,
                    "stdout": stdout,
                    "stderr": stderr,
                }),
                terminate: false,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    fn make_ctx(tmp: &std::path::Path) -> ToolContext {
        let (tx, _rx) = mpsc::channel(1);
        ToolContext {
            env: Arc::new(OsEnv::new(tmp)),
            abort: CancellationToken::new(),
            tool_use_id: "test-id".into(),
            turn_index: 0,
            assistant_message: Arc::new(llm_harness_types::AssistantMessage {
                content: vec![],
                usage: None,
                stop_reason: None,
                timestamp: chrono::Utc::now(),
                provider: None,
                api: None,
                model: None,
                error_message: None,
            }),
            update_tx: tx,
        }
    }

    #[tokio::test]
    async fn python_hello_world() {
        let tmp = tempfile::tempdir().unwrap();
        let tool = CodeExecTool::new();
        let args = serde_json::json!({
            "language": "python",
            "code": "print('hello from test')"
        });
        let result = tool.execute(args, &make_ctx(tmp.path())).await.unwrap();
        let text = match &result.content[0] {
            llm_harness_types::ContentBlock::Text { text } => text.clone(),
            _ => panic!("expected text"),
        };
        assert!(text.contains("hello from test"), "got: {text}");
        assert_eq!(result.details["exit_code"], 0);
    }

    #[tokio::test]
    async fn nonzero_exit_is_not_tool_error() {
        let tmp = tempfile::tempdir().unwrap();
        let tool = CodeExecTool::new();
        let args = serde_json::json!({
            "language": "python",
            "code": "import sys; sys.exit(1)"
        });
        let result = tool.execute(args, &make_ctx(tmp.path())).await.unwrap();
        assert_eq!(result.details["exit_code"], 1);
    }
}
