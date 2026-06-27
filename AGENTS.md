# 开发原则

- 优先使用 `llm-harness-runtime` / `llm-harness-agent`。不要在本仓库重复实现
  session、上下文构建、工具编排、hooks、trace、压缩或 provider 行为。
- `llm-tutor` 专注产品数据和 UI：知识库、文档、空间、笔记本、测验、设置，以及它们到 runtime session ID 的映射。
- 持久化对话历史时，优先使用 runtime session，例如 `AgentHarness::with_session` 和 runtime session repo。
- 如果框架 API 不顺手或缺少能力，记录到 `docs/framework-feedback.md`，不要默默搭建一套平行系统。
- 产品代码与 runtime 代码之间的 adapter 要保持薄、明确，并用边界测试覆盖。
- 完成有意义的任务后，及时提交代码。
