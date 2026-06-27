# llm-tutor

`llm-tutor` 是一个基于
[llm-harness-runtime](https://github.com/oh-my-harness/llm-harness-runtime)
构建的 AI 学习工作区。

它把聊天、RAG 知识库、分步骤解题、测验生成、联网研究，以及轻量级书籍/报告沉淀组合在一起，目标是形成一个可持续积累知识、练习和学习画像的本地 Tutor Agent。

## 使用方式

### 环境要求

- Rust 2024 edition，建议先执行 `rustup update stable`
- Node.js 20+
- 至少一个可用的 LLM Provider API Key
- 可选：Embedding Provider API Key，用于知识库/RAG 入库与检索
- 可选：Web Search API Key，用于更稳定的联网研究
- LanceDB 依赖 Protobuf 编译器 `protoc`
  - Windows 可执行：`winget install --id Google.Protobuf`

### 安装

```powershell
# 首次安装前端依赖
cd web-ui
npm install
cd ..

# 可选：后端基础测试
cargo test -p tutor-web --lib
```

### 启动

启动后端：

```powershell
cargo run -p tutor-web
```

后端默认监听：

```text
http://127.0.0.1:8080
```

另开一个终端启动前端：

```powershell
cd web-ui
npm run dev
```

然后打开：

```text
http://localhost:5173
```

### Web 端配置

打开 Web UI 左侧的 **设置** 页面，至少配置一个 LLM。

1. **LLM**
   - 新增一个模型配置。
   - 选择接口模式，例如 OpenAI-compatible、Anthropic、DeepSeek 等。
   - 填写 `base_url`、API Key、模型名、可选 chat path、上下文窗口。
   - 使用测试按钮验证模型是否可用。

2. **嵌入模型**
   - 创建知识库和 RAG 入库时需要。
   - 新增 OpenAI-compatible embedding 配置。
   - 填写 `base_url`、API Key、模型名、可选 `/v1/embeddings` path、向量维度，以及是否发送 `dimensions` 参数。
   - 使用测试按钮确认返回的向量维度。

3. **搜索**
   - 普通聊天不是必须，但 Research 模式建议配置。
   - DuckDuckGo 可作为免费兜底，但质量和可用性不稳定。
   - 也可以配置 Bing、Brave、Tavily、Serper、SerpAPI、Exa 等付费搜索服务。

4. **知识库**
   - 进入 **知识库** 页面。
   - 创建知识库，并选择嵌入模型配置。
   - 上传 PDF 或文本文件。
   - 等待解析、切分、嵌入、写入 LanceDB 完成。
   - 在聊天输入框中选择知识库后，Agent 才会按需使用 RAG 检索。

本地产品数据默认存储在项目根目录的 `.llm-tutor/` 下。

## 当前功能

| 模块 | 支持能力 |
| --- | --- |
| 聊天 | 多轮对话、流式输出、历史会话、附件、可选 RAG、网页搜索/抓取、代码执行、trace 事件。 |
| 知识库 / RAG | 创建知识库、上传文档、PDF/文本解析、LanceDB 索引、chunk 检索、在真正调用 `rag_search` 时展示引用来源。 |
| Deep Solve | 面向复杂问题的结构化解题流程，包含计划、步骤、证据、引用、最终答案和状态事件。 |
| Quiz | 可从知识库或对话材料生成测验，并支持答题流程；有来源材料时会尽量生成带证据的问题。 |
| Research | 联网搜索、读取网页、综合生成 Markdown 研究报告，并可把报告保存到书籍/笔记流程。 |
| 空间 | 承载笔记本、题库、学生画像等学习资产入口。 |
| 记忆 | L1/L2/L3 Markdown 记忆、手动更新/检查/去重工作台、来源引用、候选事实/编辑预览，以及 `read_memory` / `write_memory` 工具。 |
| 书籍 | 轻量级书籍和章节存储，用于保存整理后的报告和输出。 |

## 可选 CLI

```powershell
# 普通聊天
cargo run -p tutor-agent -- "What is integration by parts?"

# Deep Solve
cargo run -p tutor-agent -- --capability deep_solve "Evaluate the integral of x^2 from 0 to 2"
```

CLI 或环境变量驱动运行时，常用变量如下：

```powershell
$env:LLM_PROVIDER="openai"
$env:OPENAI_API_KEY="sk-..."
$env:LLM_MODEL="gpt-4o-mini"
$env:OPENAI_BASE_URL="https://api.openai.com"
$env:OPENAI_CHAT_PATH="/v1/chat/completions"
```

DeepSeek 或其他 OpenAI-compatible 网关可以使用 OpenAI-compatible 模式，并配置自己的 base URL 和模型名。

Bash/Zsh 中请使用 `export NAME=value`。

## 当前状态

`llm-tutor` 目前是一个单用户本地产品原型。核心学习闭环已经可用，但部分模块仍处于 MVP 阶段，需要继续打磨质量和体验。

已经实现：

- 基于 runtime session 的聊天和历史会话。
- WebSocket 流式输出。
- 工具和长流程的 trace/status 事件。
- UI 中配置 LLM、嵌入模型、Web 搜索。
- 知识库创建、文档上传、PDF/文本解析、LanceDB 入库和检索。
- RAG 引用来源和 source chunk 展示。
- 聊天附件，包括当前轮 PDF/文本解析。
- Deep Solve 结构化解题体验。
- 代码执行工具。
- Quiz 生成与答题流程。
- Web 搜索和网页抓取工具。
- Research 模式，可搜索、阅读并生成 Markdown 报告。
- 研究报告保存到书籍章节。
- 空间、笔记本、题库、学生画像和记忆页面。
- L1/L2/L3 Markdown 记忆凝练，支持结构化更新、检查和去重。
- 基础书籍/章节浏览。

仍处于早期：

- Research Report 还不是完整的一等数据结构。
- 书籍编辑目前只是简单章节查看，不是富文本编辑器。
- RAG chunk 策略仍比较基础，后续应升级为段落/token-aware。
- 引用质量和来源校验还需要继续增强。
- 本地持久化主要是 JSON 加 runtime session 存储，后续可能需要 SQLite。
- 多用户、鉴权、权限、部署和协作暂不在当前范围内。

## 架构

```text
web-ui (React + Vite + Tailwind)
  -> REST / WebSocket
tutor-web (Axum)
  -> SessionPool / runtime sessions
  -> tutor-agent
      |-- CapabilityRouter
      |-- Chat / Research harness runs
      |-- Deep Solve orchestrator
      |-- Quiz generation helpers
      `-- Governance / budget / audit hooks
  -> tutor-tools
      |-- rag_search
      |-- web_search
      |-- web_fetch
      `-- code_exec
  -> tutor-rag
      `-- LanceDB + embedding-backed retrieval
  -> llm-harness-runtime / llm-harness-agent
```

### Crates

```text
crates/tutor-agent   Agent 能力、提示词、能力路由、Deep Solve、Quiz 生成。
crates/tutor-tools   暴露给 Agent 的工具：RAG、网页搜索/抓取、代码执行。
crates/tutor-rag     LanceDB 入库/检索与 embedding 集成。
crates/tutor-web     Axum 服务、WebSocket、session API、知识库、Quiz、书籍。
web-ui               React 前端。
docs                 路线图、规格文档、框架反馈和功能计划。
```

## 本地数据

runtime 和产品数据默认存储在项目根目录的 `.llm-tutor/` 下。

常见文件包括：

```text
.llm-tutor/knowledge-bases.json
.llm-tutor/quizzes.json
.llm-tutor/books.json
```

向量数据由 LanceDB 管理，存放在配置的 RAG 根目录下。

## 测试

```bash
# Rust 测试
cargo test --workspace -j 1

# Agent mock integration tests
cargo test -p tutor-agent --test mock_integration -j 1

# 后端 API / store 测试
cargo test -p tutor-web --lib -j 1

# 前端构建
cd web-ui
npm run build
```

真实 provider 集成测试通常需要 API Key，默认一般会被忽略。

## 开发原则

见 [AGENTS.md](./AGENTS.md)。

简短版本：

- 优先使用 `llm-harness-runtime` / `llm-harness-agent`。
- 不要在本仓库重复实现 runtime session、上下文构建、工具编排、trace、压缩或 provider 行为。
- 本仓库聚焦产品数据和 UI：知识库、文档、空间、书籍、测验、设置、研究报告，以及它们到 runtime session 的映射。

## License

MIT
