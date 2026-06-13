# llm-tutor 开发规范

## 仓库结构

```
crates/
├── tutor-tools/   # RAG 检索、Web 搜索、代码执行工具
├── tutor-agent/   # Chat + DeepSolve 能力，CLI 入口
└── tutor-web/     # Web API（Axum，流式响应）
```

## tutor-agent 核心模块

| 文件 | 职责 |
|------|------|
| `capability.rs` | CapabilityRouter：Chat / DeepSolve / CodeExec 路由 |
| `llm_provider.rs` | LlmConfig：多 provider 配置（Anthropic / DeepSeek / OpenAI） |
| `governance.rs` | GovernanceConfig：budget + audit + approval 组装 |
| `solve_orchestrator.rs` | DeepSolve 四阶段流水线：Pre-retrieve → Plan → Solve → Synthesize |
| `chat.rs` | Chat 单轮：RAG + Web 搜索 → 回答 |

## 测试规范

mock 测试通过 `LlmConfig::anthropic("mock-model", "")` + `.with_client(mock_client)` 注入，所有 harness 共享同一个 `MockLlmClient` 实例（responses 按调用顺序消费）：

```rust
fn make_router(responses: Vec<MockResponse>, governance: GovernanceConfig) -> CapabilityRouter {
    let client = Arc::new(MockLlmClient::new(responses));
    let env = Arc::new(NoOpEnv) as Arc<dyn ExecutionEnv>;
    let llm = LlmConfig::anthropic("mock-model", "");
    CapabilityRouter::new(env, llm, governance).with_client(client)
}
```

DeepSolve 单步需要 3 个 response：Plan JSON → `FINISH: ...` → 最终合成文本。

真实 API 测试（`deep_solve_integration.rs`）默认 `#[ignore]`，需要真实 API key。

## 注意

`HarnessHooks::should_stop` 不要设为 budget adapter——budget adapter 在未超限时返回 `false`（继续），会导致无限循环。保持 `None`（默认：非工具响应后自动停止）。
