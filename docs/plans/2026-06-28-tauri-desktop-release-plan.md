# Tauri Desktop Release Plan

> Status: proposed | Date: 2026-06-28 | Scope: package `llm-tutor` as a local-first Tauri desktop application.

## 1. Decision

`llm-tutor` 的正式发布形态采用 Tauri 桌面应用。

桌面应用应保留当前 Web UI + Rust 后端的产品架构，但把启动、数据目录、配置、更新和分发体验收敛成一个普通用户可以安装和启动的软件。

## 2. Release Goal

- 用户下载安装包后，可以像普通桌面软件一样启动 `llm-tutor`。
- 用户不需要手动启动 `tutor-web` 和 `web-ui` 两个开发进程。
- 数据默认保存在本机应用数据目录，符合本地优先原则。
- LLM、Embedding、Search 等 API Key 仍由用户在设置页自行配置。
- 不内置模型密钥，不绑定单一模型服务商。

## 3. Target Shape

```text
llm-tutor desktop app
  -> Tauri shell
      -> bundled React UI
      -> managed Rust backend sidecar or embedded backend runtime
      -> local app data directory
      -> OS integration: window, tray/update later
```

第一版优先采用保守方案：

- Tauri 负责窗口、打包、应用数据目录和启动流程。
- React 前端继续由 `web-ui` 构建产物提供。
- Rust 后端优先作为 sidecar 进程随应用启动。
- 前端通过本地 HTTP/WebSocket 访问后端。

后续可以评估是否把 `tutor-web` 的 Axum 服务更深地嵌入 Tauri command/runtime，但第一版不强行重构。

## 4. Packaging Requirements

- Windows 是第一优先发布平台。
- 后续支持 macOS 和 Linux。
- 安装包应包含：
  - Tauri 应用壳。
  - 前端静态资源。
  - `tutor-web` release 构建产物或等价后端运行时。
  - 默认配置模板。
  - README / 使用说明。
  - License。
- 应用启动时自动启动本地后端。
- 应用退出时应关闭托管的后端进程。
- 后端端口应避免与用户环境冲突。
- 前端不应依赖 `npm run dev`。
- release 构建应可由脚本或 CI 重复执行。

## 5. Data and Config

- 桌面版默认数据目录应使用系统应用数据目录，而不是项目根目录 `.llm-tutor/`。
- 开发模式仍可继续使用项目根目录 `.llm-tutor/`。
- 设置页应展示当前数据目录。
- 后续应支持数据导入/导出。
- API Key 应避免写入日志和 trace。
- 后续应评估使用系统 keychain / credential store 保存敏感配置。

## 6. Implementation Phases

### Phase 1: Desktop Skeleton

- [ ] 添加 Tauri app 工程。
- [ ] 复用现有 `web-ui` 构建产物。
- [ ] 能打开桌面窗口并展示现有 UI。
- [ ] 开发模式下仍支持现有前后端分离启动。

### Phase 2: Backend Management

- [ ] 将 `tutor-web` 作为 release sidecar 构建。
- [ ] Tauri 启动时拉起 sidecar。
- [ ] Tauri 退出时关闭 sidecar。
- [ ] 支持动态选择本地端口并传给前端。
- [ ] 后端使用桌面应用数据目录。

### Phase 3: User-Ready Packaging

- [ ] 生成 Windows 安装包。
- [ ] 生成 Windows portable 包，作为调试和轻量分发选项。
- [ ] 补充桌面版 README。
- [ ] 增加 release 构建脚本。
- [ ] 在 CI 中增加桌面包构建检查。

### Phase 4: Desktop Polish

- [ ] 首次启动引导用户配置 LLM。
- [ ] 设置页展示版本、数据目录、后端状态。
- [ ] 支持打开数据目录。
- [ ] 支持检查更新。
- [ ] 评估自动更新。

## 7. Open Questions

- 后端采用 sidecar 还是嵌入式 Axum runtime。
- 本地端口分配和前端发现机制如何设计。
- 是否需要托盘常驻。
- API Key 是否第一版就接入系统 keychain。
- Windows 安装器优先 NSIS、MSI，还是先只做 portable。

## 8. Non-Goals for First Release

- 不做云端 SaaS。
- 不做多用户权限系统。
- 不内置模型服务或代理服务。
- 不强制所有数据同步到云端。
- 不在第一版重构全部后端为 Tauri command。

## 9. Acceptance

- 用户下载安装包后可以启动桌面应用。
- 用户可以在桌面应用中完成 LLM 配置。
- 用户可以创建会话、发送消息并看到流式回复。
- 用户可以创建知识库并上传文档。
- 用户重启应用后，本地数据仍可恢复。
- 开发者可以用一条明确命令构建桌面 release 包。
