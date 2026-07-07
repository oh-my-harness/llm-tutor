# Tutor Agent v0.1 设计文档

> Historical note (2026-07-07): this document records the original v0.1
> design. Current implementation has migrated Deep Solve, Quiz, and Memory to
> `llm-harness-runtime` `WorkflowEngine` / `HarnessBuilder`; legacy
> `PhaseManager`, `ReplanHook`, `ReplanTool`, `SolveContext`, and direct
> `BudgetControlAdapter` wiring are no longer current. See
> `docs/framework-feedback.md` for the authoritative runtime migration status.

> 状态: 设计阶段 | 仓库: `/Users/hhl/Documents/projs/tutor_agent` | runtime 依赖: `llm-harness-runtime` v0.2+

---

## 目录

1. [背景与目标](#1-背景与目标)
2. [整体架构](#2-整体架构)
3. [Capability 设计](#3-capability-设计)
4. [编排层设计](#4-编排层设计)
5. [流式输出（WebSocket）](#5-流式输出websocket)
6. [审计与预算](#6-审计与预算)
7. [Web UI](#7-web-ui)
8. [实现计划](#8-实现计划)
9. [v0.1 范围边界](#9-v01-范围边界)

---

## 1. 背景与目标

### 1.1 为什么做 Tutor Agent

`llm-harness-runtime` 需要一个非 coding agent 场景来验证框架通用性。教学场景天然需要框架的治理能力：

| 框架特性 | 教学场景需求 |
|---------|------------|
| Sandbox 隔离 | 执行学生代码（v0.2） |
| BudgetControl | 学生预算有限，超支必须硬停 |
| AuditSink | 学习轨迹可追溯 |
| HumanApproval | code_exec 审批门控 |
| PrepareNextTurnHook | Solve step 内动态工具切换 |
| BeforeToolCallHook | REPLAN 工具拦截 |

参考项目：[DeepTutor](https://github.com/HKUDS/DeepTutor)（Python 实现），借鉴其 SolvePipeline 的多阶段设计和 StreamBus 流式架构。

### 1.2 v0.1 交付目标

**三个 Capability：**
- **Chat** — 基于 RAG 知识库的问答辅导
- **Deep Solve** — 多阶段引导解题（Pre-retrieve → Plan → Solve → Synthesize）
- **Code Exec** — 沙箱内安全执行代码并解释结果

**验证目标：**
1. 多阶段状态机（Deep Solve 四阶段 + REPLAN 回溯）能否稳定运行
2. BeforeToolCallHook 拦截 `replan()` 驱动控制流回溯
3. BudgetControlAdapter 跨多个 harness 累加成本、超限硬停
4. WebSocket 流式输出三类事件（content / trace / status）
5. AuditSink 完整记录学习轨迹

---

## 2. 整体架构

### 2.1 仓库与 crate 结构

```
tutor_agent/
├── Cargo.toml                    (workspace)
├── crates/
│   ├── tutor-tools/              (工具实现)
│   │   └── src/
│   │       ├── rag_search.rs
│   │       ├── web_search.rs
│   │       ├── code_exec.rs
│   │       └── lib.rs
│   ├── tutor-agent/              (编排核心)
│   │   └── src/
│   │       ├── capability.rs     # CapabilityRouter
│   │       ├── solve_orchestrator.rs  # SolveOrchestrator + SolveContext
│   │       ├── phase_manager.rs  # PhaseManager (PrepareNextTurnHook)
│   │       ├── replan_hook.rs    # ReplanHook (BeforeToolCallHook)
│   │       ├── verifier.rs       # TaskVerifier
│   │       └── lib.rs
│   └── tutor-web/                (HTTP server)
│       └── src/
│           ├── routes/
│           ├── stream.rs         # TutorStream (WebSocket event bus)
│           ├── session.rs
│           └── main.rs
├── web-ui/                       (Vite + React + Tailwind)
│   ├── src/
│   │   ├── components/           # ChatBox, CapabilitySelector, BudgetPanel, TracePanel
│   │   └── App.tsx
│   └── package.json
└── docs/
    └── specs/
        └── 2026-06-13-tutor-agent-v0.1-design.md  (本文档)
```

**依赖方向：**

```
tutor-web → tutor-agent → tutor-tools → llm-harness-runtime
```

**v0.1 引入的 runtime crate：**

```toml
llm-harness-runtime             # 核心：ToolRegistry、TaskRunner、BudgetControlAdapter、HumanApprover、AuditSink
llm-harness-runtime-sandbox-os  # OsEnvSandbox（v0.1 唯一沙箱后端）
llm-harness-runtime-audit-jsonl # JSONL 审计日志
llm-harness-runtime-auth        # EnvAuthHook
```

### 2.2 运行时架构

```
┌────────────────────────────────────────┐
│         web-ui (Vite + React)          │
│  ChatBox | CapabilitySelector          │
│  BudgetPanel | TracePanel              │
└──────────────────┬─────────────────────┘
                   │ WebSocket / REST
┌──────────────────v─────────────────────┐
│         tutor-web (axum)               │
│  WS handler → TutorStream             │
│  REST routes | Session pool            │
└──────────────────┬─────────────────────┘
                   │
┌──────────────────v─────────────────────┐
│         tutor-agent (编排)             │
│                                        │
│  CapabilityRouter                      │
│    ├── Chat capability                 │
│    ├── DeepSolve (SolveOrchestrator)  │
│    └── CodeExec capability             │
│                                        │
│  PhaseManager (PrepareNextTurnHook)    │
│  ReplanHook   (BeforeToolCallHook)     │
│  TaskVerifier                          │
└──────────────────┬─────────────────────┘
                   │ Tool trait
┌──────────────────v─────────────────────┐
│         tutor-tools                    │
│  rag_search | web_search | code_exec  │
│                  ↕                     │
│       OsEnvSandbox (code_exec)         │
└──────────────────┬─────────────────────┘
                   │
┌──────────────────v─────────────────────┐
│         llm-harness-runtime            │
│  TaskRunner | ToolRegistry             │
│  BudgetControlAdapter | AuditSink      │
│  HumanApprover | AuthHook              │
└────────────────────────────────────────┘
```

---

## 3. Capability 设计

### 3.1 工具清单

```
T0 读取型（无副作用）
├── rag_search(query, kb) → str          — RAG 知识库检索
├── web_search(query) → str             — 网络搜索
└── get_session_context() → SessionCtx  — 读取学习进度（v0.2）

T1 可逆写入
└── save_note(content, topic) → NoteId  — 保存学习笔记（v0.2）

T2 外部副作用
├── code_exec(language, code) → ExecResult  — OsEnvSandbox 执行（v0.1）
│                                             BwrapSandbox/SeatbeltSandbox（v0.2）
└── replan(reason) → !                  — 触发 REPLAN 回溯（被 BeforeToolCallHook 拦截）
```

### 3.2 工具激活矩阵

| Capability / 阶段 | 活跃工具 |
|-----------------|---------|
| Chat - Explore | `rag_search`, `web_search` |
| Chat - Respond | _(纯 LLM 输出，无工具)_ |
| Deep Solve - Pre-retrieve | `rag_search` |
| Deep Solve - Plan | _(纯 LLM 输出，无工具)_ |
| Deep Solve - Solve step | `rag_search`, `web_search`, `code_exec`, `replan` |
| Deep Solve - Synthesize | _(纯 LLM 输出，无工具)_ |
| Code Exec | `code_exec` |

### 3.3 Chat Capability

```
loop:
  [Harness] Explore: rag_search + web_search → Respond
  用户追问 → 继续循环
完成: save_note（可选）
```

Hook 配置：
- `AfterTurnHook` → AuditSink 记录问答对

### 3.4 Deep Solve Capability（核心）

```
[Harness A] Pre-retrieve（有 KB 时）
  工具: rag_search
  输出: SUMMARY 文本 → 写入 SolveContext.kb_summary

[Harness B] Plan
  工具: 无
  输出: JSON { analysis: str, steps: [{id, goal}] } → SolveContext.plan
  初次: 基于 question + kb_summary 规划
  REPLAN: 携带 previous_plan + replan_reason 重新规划

for each step in plan.steps:
  [Harness C] Solve step
    工具: rag_search, web_search, code_exec, replan
    流程: THINK（推理）→ TOOL calls（工具） → FINISH（输出本步结论）
    REPLAN 路径:
      agent 调用 replan(reason)
      → BeforeToolCallHook (ReplanHook) 拦截：
          写入 SolveContext.replan_reason = reason（共享状态）
          返回 Deny(ToolResult)（工具体不执行，harness 正常 Settled）
      → SolveOrchestrator.run_solve_steps() 检测共享状态中 replan_reason 非空
      → 提前返回，外层 loop 调用 should_replan() → reset_for_replan()
      → 重启 Harness B（replan_count += 1，plan + step_results 清空）
      → max_replans = 2，should_replan() 返回 false 时退出 loop，继续 Synthesize

[Harness D] Synthesize
  工具: 无
  输入: question + step_results（所有步骤 FINISH 内容）
  输出: 最终答案 + 摘要
  后置: TaskVerifier 判定（可选预设答案比对）
```

**SolveContext（阶段间共享状态）：**

```rust
struct SolveContext {
    question: String,
    kb_summary: Option<String>,
    plan: Option<Plan>,
    step_results: Vec<StepResult>,
    replan_count: usize,
    replan_reason: Option<String>,  // 由 ReplanHook 写入
    max_replans: usize,             // 默认 2
}

impl SolveContext {
    /// 是否应触发 REPLAN：reason 已设置且未达上限。
    fn should_replan(&self) -> bool {
        self.replan_reason.is_some() && self.replan_count < self.max_replans
    }

    /// 清理状态准备新一轮 Plan（保留 question/kb_summary）。
    fn reset_for_replan(&mut self) {
        self.plan = None;
        self.step_results.clear();
        self.replan_count += 1;
        self.replan_reason = None;
    }
}

struct Plan {
    analysis: String,
    steps: Vec<PlanStep>,
}

struct PlanStep { id: String, goal: String }
struct StepResult { step_id: String, finish_text: String }
```

### 3.5 Code Exec Capability

```
[Harness] 
  工具: code_exec
  流程: agent 调用 code_exec → OsEnvSandbox 执行 → agent 解释结果
```

安全约束（v0.1 OsEnvSandbox）：
```rust
ResourceLimits {
    timeout: Some(Duration::from_secs(30)),
    max_memory_mb: None,   // OsEnvSandbox 不强制（v0.2 沙箱后端才隔离）
}
```

---

## 4. 编排层设计

### 4.1 CapabilityRouter

```rust
pub struct CapabilityRouter {
    env: Arc<dyn ExecutionEnv>,
    model: String,
    governance: GovernanceConfig,   // 聚合 budget + audit + approval（跨 harness 共享）
    stream: Option<Arc<TutorStream>>, // WebSocket 事件总线（Web 模式），CLI 模式为 None
}

impl CapabilityRouter {
    pub async fn run(&self, capability: Capability, question: &str) -> Result<String>;
}
```

`governance.budget` 跨所有 harness 共用同一个 `BudgetControlAdapter` 实例，保证 session 内各阶段累加。
`stream` 在 CLI 模式下为 `None`（不推送流式事件），Web 模式下注入 `TutorStream`。

### 4.2 SolveOrchestrator

```rust
pub struct SolveOrchestrator {
    context: SolveContext,
    env: Arc<dyn ExecutionEnv>,
    model: String,
    governance: GovernanceConfig,
    stream: Option<Arc<TutorStream>>,
}

impl SolveOrchestrator {
    /// question 在构造时绑定，kb 在 run() 时传入。
    pub fn new(
        question: impl Into<String>,
        env: Arc<dyn ExecutionEnv>,
        model: impl Into<String>,
        governance: GovernanceConfig,
        stream: Option<Arc<TutorStream>>,
    ) -> Self;

    /// 运行完整流水线：[Pre-retrieve] → Plan → (Solve → [REPLAN])* → Synthesize。
    pub async fn run(&mut self, kb: Option<&str>) -> Result<String> {
        // 1. Pre-retrieve（有 KB 时）
        if let Some(kb_text) = kb {
            self.run_pre_retrieve(kb_text).await?;
        }
        
        // 2. Plan loop（支持 REPLAN 回溯）
        loop {
            self.run_plan().await?;
            self.run_solve_steps().await?;
            
            if !self.context.should_replan() { break; }
            self.context.reset_for_replan();
        }
        
        // 3. Synthesize
        self.run_synthesize().await
    }
}
```

### 4.3 PhaseManager（职责收窄）

```rust
/// PrepareNextTurnHook: 仅控制 Solve step 内的工具白名单。
/// 不驱动外层阶段切换（外层由 SolveOrchestrator 顺序调用控制）。
/// 使用 `active_tools` 从已注册工具中筛选手集（而非 `tools` 替换全部）。
pub struct PhaseManager {
    allowed_tools: Vec<String>,
}

impl PrepareNextTurnHook for PhaseManager {
    fn prepare<'a>(
        &'a self,
        _ctx: PrepareNextTurnCtx<'a>,
    ) -> BoxFuture<'a, Result<NextTurnDirective, AgentError>> {
        let tools: HashSet<String> = self.allowed_tools.iter().cloned().collect();
        Box::pin(async move {
            Ok(NextTurnDirective {
                active_tools: Some(tools),
                ..Default::default()
            })
        })
    }
}
```

### 4.4 ReplanHook

```rust
/// BeforeToolCallHook: 拦截 replan() 工具调用，触发 REPLAN 回溯。
/// 通过 Arc<Mutex<SolveContext>> 共享状态写入 replan_reason，
/// SolveOrchestrator 在每个 Solve step 结束后检测该标志。
pub struct ReplanHook {
    context: Arc<Mutex<SolveContext>>,
}

impl BeforeToolCallHook for ReplanHook {
    fn on_call<'a>(
        &'a self,
        ctx: BeforeToolCallCtx<'a>,
    ) -> BoxFuture<'a, BeforeToolCallDecision> {
        Box::pin(async move {
            if ctx.tool_name != "replan" {
                return BeforeToolCallDecision::Allow;
            }
            let reason = ctx.args["reason"].as_str().unwrap_or("").to_string();
            self.context.lock().unwrap().replan_reason = Some(reason);
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
```

---

## 5. 流式输出（WebSocket）

### 5.1 TutorStream 事件类型

```
content  — agent 文本 chunk（流式输出）
trace    — 内部事件（工具调用、阶段切换、REPLAN 等），渲染到 TracePanel
status   — 状态通知（budget_warning、phase_change、approval_request、error）
```

### 5.2 Deep Solve 典型事件序列

```json
{"type":"trace",  "payload":{"kind":"phase_start","phase":"pre_retrieve"}}
{"type":"trace",  "payload":{"kind":"tool_call","tool":"rag_search","query":"..."}}
{"type":"trace",  "payload":{"kind":"phase_start","phase":"plan"}}
{"type":"content","payload":{"text":"Step 1: 分析积分...","chunk":true}}
{"type":"trace",  "payload":{"kind":"phase_start","phase":"solve_step","step":1}}
{"type":"trace",  "payload":{"kind":"tool_call","tool":"code_exec","language":"python"}}
{"type":"content","payload":{"text":"执行结果：4.0\n","chunk":true}}
{"type":"trace",  "payload":{"kind":"replan","reason":"符号计算更精确","count":1}}
{"type":"trace",  "payload":{"kind":"phase_start","phase":"plan","is_replan":true}}
{"type":"status", "payload":{"kind":"budget_warning","remaining_usd":0.50}}
{"type":"content","payload":{"text":"最终答案：8/3","chunk":false}}
```

### 5.3 后端实现

```rust
// tutor-web/src/stream.rs
pub struct TutorStream {
    tx: tokio::sync::mpsc::Sender<StreamEvent>,
}

pub enum StreamEvent {
    Content { text: String, chunk: bool },
    Trace   { kind: String, payload: serde_json::Value },
    Status  { kind: String, payload: serde_json::Value },
}

impl TutorStream {
    pub async fn content(&self, text: &str, chunk: bool);
    pub async fn trace(&self, kind: &str, payload: impl Serialize);
    pub async fn status(&self, kind: &str, payload: impl Serialize);
}
```

### 5.4 前端

```
useWebSocket hook 订阅 /ws/sessions/:id
  content  → 追加到 ChatBox 聊天气泡（逐 token 更新）
  trace    → 追加到 TracePanel 侧边面板（折叠展开）
  status   → budget_warning → BudgetPanel 更新
           → approval_request → 弹出 ApprovalDialog
           → error → 错误提示
```

---

## 6. 审计与预算

### 6.1 治理配置

```rust
/// 聚合 session 级治理组件（预算 + 审计 + 人工审批），跨所有 harness 共享。
pub struct GovernanceConfig {
    pub budget: Arc<BudgetControlAdapter>,
    pub audit: Option<Arc<dyn AuditSink>>,
    pub approval: Option<Arc<HumanApprovalWrapper>>,
    pub require_code_exec_approval: bool,
}
```

### 6.2 预算控制

```rust
// 全局 session 预算，跨所有 harness 共用
let budget = Arc::new(BudgetControlAdapter::new(
    pricing_provider,
    2.00,   // max_total_cost per session，单位 USD
    None,   // 可选 token 上限
));
// BudgetControlAdapter 同时实现 AfterProviderResponseHook（累加成本）
// 和 ShouldStopHook（超限返回 ShouldStop::Yes，触发 harness Settled）
```

### 6.3 审计事件

| 事件 | 触发点 | 关键字段 |
|-----|-------|---------|
| `SessionStart` | session 创建 | capability, kb_name |
| `PhaseTransition` | 阶段切换 | from_phase, to_phase |
| `ToolCall` | 工具调用 | tool_name, args_summary, success |
| `CodeExec` | code_exec 专项 | language, exit_code, duration_ms |
| `Replan` | REPLAN 触发 | reason, replan_count |
| `BudgetExceeded` | 超限 | cost_usd, limit_usd |

v0.1 不做 hash 链完整性验证（留 v0.2）。

---

## 7. Web UI

### 7.1 技术栈

- **前端**: Vite + React 19 + TypeScript + Tailwind CSS
- **后端**: axum（REST + WebSocket）
- 后续视需求升级 Next.js

### 7.2 API

```
REST (Phase 4 实现):
  POST /api/sessions               — 创建 session（capability, kb?）
  GET  /api/sessions/:id           — 获取 session 详情

REST (v0.2):
  GET  /api/sessions/:id/cost      — 当前成本
  GET  /api/sessions/:id/audit     — 审计日志
  GET  /api/kb                     — 列出知识库

WebSocket (Phase 4 实现):
  WS /ws/sessions/:id
    客户端 → 服务端: { type: "message", content: "..." }
    服务端 → 客户端: content / trace / status 事件（见 §5）
    服务端 → 客户端: { type: "approval_request", tool: "code_exec", ... }
    客户端 → 服务端: { type: "approval_response", approved: true/false }
```

### 7.3 前端组件

```
App
├── CapabilitySelector   — Chat / Deep Solve / Code Exec 切换
├── KbSelector           — 知识库选择（有 KB 时显示）
├── ChatBox              — 消息历史 + 流式追加
├── TracePanel           — 工具调用、阶段切换的折叠式日志
├── BudgetPanel          — 实时余额显示
└── ApprovalDialog       — code_exec 审批弹窗
```

---

## 8. 实现计划

### Phase 1 — 骨架（Week 1-2）

```
目标: Chat capability 端到端可用

1. [ ] 创建 tutor_agent workspace（Cargo.toml + crates/）
2. [ ] tutor-tools: rag_search, web_search, code_exec (OsEnvSandbox)
3. [ ] tutor-agent: CapabilityRouter + Chat capability
4. [ ] CLI 验证: chat 消息 → rag_search → 回答

验证标准:
  - rag_search 从知识库检索内容
  - Chat Explore → Respond 状态机正常流转
  - AuditSink 记录 SessionStart + ToolCall 事件
```

### Phase 2 — Deep Solve（Week 2-3）

```
目标: 四阶段编排 + REPLAN 回溯

5. [ ] SolveContext + SolveOrchestrator 骨架
6. [ ] Pre-retrieve harness（rag_search → SUMMARY）
7. [ ] Plan harness（JSON 步骤输出）
8. [ ] Solve step harness（THINK/TOOL/FINISH 循环）
9. [ ] replan() 工具 + ReplanHook（BeforeToolCallHook 拦截）
10. [ ] Synthesize harness + TaskVerifier
11. [ ] 集成测试: Pre-retrieve → Plan → Solve → Synthesize 全流转
12. [ ] 集成测试: REPLAN 回溯（max_replans 上限生效）

验证标准:
  - REPLAN 触发后 Plan 重启，携带 replan_reason
  - replan_count 超限后继续执行 Synthesize（不卡死）
```

### Phase 3 — 治理（Week 3-4）

```
目标: 预算 + 审计 + 人工审批

13. [ ] BudgetControlAdapter 跨 harness 累加（session 级共享）
14. [ ] 超限验证: 预算耗尽 → task Failed → 前端收到 budget_exceeded
15. [ ] AuditSink JSONL: 所有关键事件记录
16. [ ] HumanApprover: code_exec 触发审批（可配置关闭）
```

### Phase 4 — Web（Week 4-5）

```
目标: 浏览器完整使用

17. [ ] tutor-web: TutorStream + WebSocket handler
18. [ ] tutor-web: REST routes（session CRUD）
19. [ ] web-ui: Vite + React 脚手架
20. [ ] web-ui: ChatBox + CapabilitySelector + BudgetPanel
21. [ ] web-ui: TracePanel（折叠展开工具调用记录）
22. [ ] web-ui: ApprovalDialog（code_exec 审批弹窗）
23. [ ] 端到端: 浏览器完成 Deep Solve 全流程

验证标准:
  - content 流式逐 token 渲染
  - TracePanel 实时显示阶段切换和工具调用
  - BudgetPanel 实时更新余额
  - ApprovalDialog 触发后 Deny 阻断工具执行
```

### Phase 5 — 文档 + 框架反馈（Week 5-6）

```
目标: 验证报告

24. [ ] README + 快速上手指南
25. [ ] 框架验证报告:
    - 使用了哪些 hook，使用体验
    - 发现了哪些框架不足或 bug
    - 向 llm-harness-runtime 提 issue/PR
```

---

## 9. v0.1 范围边界

**v0.1 明确不做：**

- 沙箱隔离（bwrap / seatbelt）— 推到 v0.2
- Multi-user 隔离 — 单用户部署
- Deep Question（自动出题）
- Deep Research（多子 agent 并行研究）
- Math Animator
- Three-layer Memory（学习记忆系统）
- TutorBot（持久化自治 AI 导师）
- Explain judge（Solve 后补充概念解释）
- 多语言支持（中/英切换）
- AuditSink hash 链完整性验证

**v0.2 预留：**

- 沙箱后端（BwrapSandbox / SeatbeltSandbox）
- Multi-user session 隔离
- `save_note` 工具实现（学习笔记持久化）
- `get_session_context` 工具实现（学习进度读取）
- Explain judge
- AuditSink hash 链验证
- REST `/cost`、`/audit`、`/kb` 端点

---

## 附录：与 llm-harness-runtime 的分工

| 层 | 负责 | 具体 |
|----|------|------|
| **llm-harness-runtime** | 机制 | TaskRunner、Sandbox trait、ToolRegistry、AuditSink、BudgetControlAdapter、HumanApprover、BeforeToolCallHook |
| **tutor-agent** | 策略 | SolveOrchestrator 阶段编排、ReplanHook 拦截逻辑、PhaseManager 工具白名单、TaskVerifier 判定规则 |
| **tutor-web / web-ui** | 体验 | TutorStream、REST API、WebSocket、ChatBox、BudgetPanel、TracePanel、ApprovalDialog |

**原则：不在 tutor-agent 中重新实现 runtime 已有能力。** 预算控制只配置 `BudgetControlAdapter`，不自写 cost tracking；审计只配置 `AuditSink`，不自写日志。
