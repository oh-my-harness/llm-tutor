# llm-tutor / Tutor Agent

`llm-tutor` 是一个基于
[`llm-harness-runtime`](https://github.com/oh-my-harness/llm-harness-runtime)
构建的本地优先 AI 学习工作区。桌面产品名为 **Tutor Agent**。

它把多轮聊天、复杂问题求解、联网研究、测验、RAG 知识库、Markdown Notebook 和学习记忆组合在一个可持久化的桌面应用中。

> 当前版本：`0.1.3`
>
> 当前阶段：单用户本地产品原型；核心闭环可用，桌面发布仍在持续人工 QA。

## 文档

- [使用手册](./MANUAL.md)：首次配置和全部用户功能
- [产品需求规格](./docs/specs/2026-06-26-product-requirements-spec.md)
- [桌面发布计划](./docs/plans/2026-06-28-tauri-desktop-release-plan.md)
- [桌面 QA 清单](./docs/qa/desktop-v0.1.md)
- [框架反馈](./docs/framework-feedback.md)
- [开发原则](./AGENTS.md)
- [更新记录](./CHANGELOG.md)

## 当前功能

| 模块 | 当前能力 |
| --- | --- |
| Chat | runtime session、多轮历史、流式输出、附件、模型/模式/来源选择、`@` 空间引用、消息操作栏、trace。 |
| Deep Solve | 分步骤复杂问题求解、计划与状态、证据和代码验证。 |
| Research | 普通对话确认需求后，通过 `create_research_report` 启动独立 workflow；报告、来源和运行状态可恢复。 |
| Quiz | 普通对话确认要求后，通过 `create_quiz` 启动独立 workflow；生成可恢复、可继续答题的 Quiz 卡片。 |
| Knowledge / RAG | 创建知识库、绑定 embedding、PDF/文本入库、LanceDB 检索、引用和来源导航。 |
| Notebook | Markdown 文件树、文件夹、编辑、Wiki Link、反向链接、导入/导出、外部 Vault、生成内容保存。 |
| Space | 题库、来源筛选、学生画像和跨模块学习资产入口。 |
| Memory | L1/L2/L3 Markdown 记忆、模型/模式可选的维护 workflow、更新/检查/去重、来源引用和撤销。 |
| Desktop | Tauri 原生窗口、托管 `tutor-web` sidecar、系统文件对话框、桌面剪贴板/右键菜单、外部链接。 |
| Appearance | `cool-light` 与 `graphite-dark` 主题，中英文界面。 |

“辅导机器人”独立页面目前仍是占位入口，后续定位为结合学习目标、记忆、资料和测验反馈的持久化个性化导师。Books 兼容后端存储仍存在，但不再是当前侧边栏中的主要用户工作区。

## 快速开始

### 桌面安装包

发布产物由 [GitHub Actions](./.github/workflows/release-desktop.yml) 构建：

- `Tutor-Agent-v<version>-windows-x64-setup.exe`
- `Tutor-Agent-v<version>-windows-x64.msi`
- `Tutor-Agent-v<version>-macos-x64.dmg`
- `Tutor-Agent-v<version>-macos-arm64.dmg`

版本标签发布后可从项目的
[GitHub Releases](https://github.com/oh-my-harness/llm-tutor/releases)
获取对应产物。桌面应用会自动启动本地后端，无需另行运行服务。

### 开发环境

推荐环境：

- Rust stable，Rust 2024 edition
- Node.js 22
- Tauri CLI 2.x
- Protobuf 编译器 `protoc`
- 至少一个可用的 LLM API Key

Windows 可通过 Chocolatey 安装 Protobuf：

```powershell
choco install protoc -y
```

安装前端依赖：

```powershell
npm ci --prefix web-ui
```

### 启动桌面开发模式

```powershell
cargo tauri dev
```

该命令会构建后端、启动 Vite，并由 Tauri 拉起 `tutor-web` sidecar。

### 启动浏览器开发模式

终端一：

```powershell
cargo run -p tutor-web
```

终端二：

```powershell
npm run dev --prefix web-ui
```

访问 `http://127.0.0.1:5173`。后端默认监听 `127.0.0.1:8080`。

## 首次配置

进入应用左侧“设置”：

1. 在“LLM”中添加 OpenAI-compatible 或 Anthropic Messages 配置，并运行连接测试。
2. 如需知识库，在“嵌入模型”中配置并测试 embedding 服务。
3. 如需稳定联网研究，在“搜索”中配置搜索服务。
4. 在“笔记本”中决定使用应用本地 Notebook，还是绑定外部 Markdown Vault。
5. 在“能力”中设置会话预算和工具审批策略。
6. 在“外观”中选择界面语言和浅色/深色主题。

详细字段和操作流程见 [MANUAL.md](./MANUAL.md)。

## 架构

```text
Tutor Agent desktop
  -> Tauri shell
      -> React / Vite UI
      -> managed tutor-web sidecar
          -> REST + WebSocket
          -> runtime sessions
          -> tutor-agent
              -> chat / deep solve
              -> quiz / research / memory workflows
              -> llm-harness-runtime / llm-harness-agent
          -> tutor-tools
              -> rag_search / web_search / web_fetch
              -> code_exec / read_memory / write_memory
          -> tutor-rag
              -> LanceDB + embedding retrieval
          -> local product stores
              -> Notebook / Quiz / Memory / Settings / Knowledge
```

### 工作区结构

```text
crates/tutor-agent   Agent 能力路由、提示词及 Quiz/Research/Memory workflow。
crates/tutor-tools   RAG、搜索、抓取、代码执行和记忆工具。
crates/tutor-rag     LanceDB 入库、检索和 embedding 集成。
crates/tutor-web     Axum API、WebSocket、session 映射和产品数据存储。
src-tauri            Tauri 桌面壳和 sidecar 生命周期管理。
web-ui               React 19、Vite 8、Tailwind CSS 前端。
docs                 产品规格、计划、QA 和框架反馈。
scripts              开发、版本、桌面构建和 QA 脚本。
```

## 数据存储

浏览器/源码开发模式默认使用：

```text
<repo>/.llm-tutor/
```

可通过环境变量或后端参数覆盖：

```powershell
$env:LLM_TUTOR_HOME="D:\TutorData"
cargo run -p tutor-web -- --data-dir "D:\TutorData"
```

桌面发布版使用操作系统应用数据目录，Tauri 启动时把该路径传给 sidecar。应用内可在“设置 > 能力”查看并打开准确目录。

主要数据：

```text
settings.json
sessions/
knowledge-bases.json
quizzes.json
books.json
notebook/
memory/
rag/
workflow-sessions/
```

当前 API Key 保存在本地 `settings.json`，尚未接入系统钥匙串。不要提交或共享 `.llm-tutor/` 和桌面应用数据目录。

## 测试

Rust workspace：

```powershell
cargo test --workspace -j 1
```

Agent mock integration：

```powershell
cargo test -p tutor-agent --test mock_integration -j 1
```

后端 API/store：

```powershell
cargo test -p tutor-web --lib -j 1
```

前端：

```powershell
npm test --prefix web-ui
npm run build --prefix web-ui
```

真实 Provider 集成测试需要 API Key，默认可能被忽略。

## 桌面构建与发布

本地 Windows 构建：

```powershell
.\scripts\build-desktop.ps1
```

只构建 release 可执行文件：

```powershell
.\scripts\build-desktop.ps1 -NoBundle
```

指定 bundle：

```powershell
.\scripts\build-desktop.ps1 -Bundles nsis
```

自动化 smoke QA：

```powershell
.\scripts\qa-desktop.ps1
```

版本同步：

```powershell
.\scripts\bump-version.ps1 0.1.4
```

GitHub 发布工作流在 `v*` 标签和手动 `workflow_dispatch` 下构建 Windows x64、macOS x64 和 macOS arm64 产物。CI 需要 `PRIVATE_DEPS_TOKEN` 读取私有 Git 依赖。

## 当前限制

- 单用户、本地优先；没有账号、权限、云同步或协作。
- 辅导机器人独立页面仍是占位页。
- 运行中的 workflow 在应用进程重启后尚不能保证从中断点续跑。
- API Key 暂存于本地 JSON，系统钥匙串尚未实现。
- Linux 安装包和自动更新尚未实现。
- RAG 切分、引用验证和桌面安装包 QA 仍需持续完善。

更多用户侧说明见 [使用手册](./MANUAL.md)。

## 开发原则

项目优先使用 `llm-harness-runtime` / `llm-harness-agent` 提供的 session、上下文、工具编排、hook、trace、compaction 和 provider 行为。`llm-tutor` 聚焦产品数据与 UI，不在仓库内建立平行 runtime。

完整原则见 [AGENTS.md](./AGENTS.md)。框架缺口记录在 [docs/framework-feedback.md](./docs/framework-feedback.md)。

## License

MIT
