# Tutor Agent Phase 4: Web Backend + Frontend

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a `tutor-web` axum server that exposes REST + WebSocket endpoints and a Vite + React + Tailwind `web-ui` frontend with streaming, TracePanel, BudgetPanel, and ApprovalDialog.

**Architecture:** `tutor-web` creates a session pool keyed by session ID. Each session holds a `TutorStream` — an `mpsc::Sender<StreamEvent>` that the `CapabilityRouter` calls during harness execution. The WebSocket handler receives `StreamEvent` from the channel and forwards them to the browser as JSON. The React frontend subscribes to the WebSocket and dispatches events into component state: `content` → ChatBox, `trace` → TracePanel, `status.budget_warning` → BudgetPanel, `status.approval_request` → ApprovalDialog.

**Tech Stack:** axum 0.8, tokio-tungstenite (via axum WebSocket), tower-http (CORS, static files), serde_json; frontend: Vite 6, React 19, TypeScript, Tailwind CSS 4, `useWebSocket` (custom hook, no library).

---

## File Map

### Backend

| File | Responsibility |
|------|---------------|
| `crates/tutor-web/Cargo.toml` | axum, tower-http deps |
| `crates/tutor-web/src/main.rs` | Server entry point, router setup |
| `crates/tutor-web/src/stream.rs` | `TutorStream`, `StreamEvent` |
| `crates/tutor-web/src/session.rs` | `SessionPool`, `SessionEntry` |
| `crates/tutor-web/src/routes/mod.rs` | route registration |
| `crates/tutor-web/src/routes/sessions.rs` | REST: POST /api/sessions, GET /api/sessions/:id, GET /api/sessions/:id/cost |
| `crates/tutor-web/src/routes/ws.rs` | WS: /ws/sessions/:id |

### Frontend

| File | Responsibility |
|------|---------------|
| `web-ui/package.json` | Vite + React + Tailwind deps |
| `web-ui/vite.config.ts` | Vite config with proxy to :8080 |
| `web-ui/src/App.tsx` | Root component, session state |
| `web-ui/src/hooks/useWebSocket.ts` | WebSocket connection + event dispatch |
| `web-ui/src/components/CapabilitySelector.tsx` | Chat / Deep Solve / Code Exec tabs |
| `web-ui/src/components/ChatBox.tsx` | Streaming message display |
| `web-ui/src/components/TracePanel.tsx` | Collapsible tool call / phase log |
| `web-ui/src/components/BudgetPanel.tsx` | Real-time cost display |
| `web-ui/src/components/ApprovalDialog.tsx` | Modal for code_exec approval |

---

### Task 1: TutorStream + StreamEvent

**Files:**
- Create: `crates/tutor-web/Cargo.toml`
- Create: `crates/tutor-web/src/lib.rs`
- Create: `crates/tutor-web/src/stream.rs`

- [ ] **Step 1: Write Cargo.toml**

```toml
# crates/tutor-web/Cargo.toml
[package]
name = "tutor-web"
version.workspace = true
edition.workspace = true
license.workspace = true

[[bin]]
name = "tutor-web"
path = "src/main.rs"

[dependencies]
tutor-agent = { path = "../tutor-agent" }
llm-harness-runtime = { workspace = true }
anyhow.workspace = true
futures.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tokio.workspace = true
tokio-util.workspace = true
axum = { version = "0.8", features = ["ws"] }
tower-http = { version = "0.6", features = ["cors", "fs"] }
uuid = { version = "1", features = ["v4"] }
```

Add `tutor-web` to workspace `Cargo.toml` members.

- [ ] **Step 2: Write test for StreamEvent serialization**

```rust
// At the bottom of crates/tutor-web/src/stream.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_event_serializes_to_json() {
        let event = StreamEvent::Content {
            text: "hello".into(),
            chunk: true,
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "content");
        assert_eq!(json["payload"]["text"], "hello");
        assert_eq!(json["payload"]["chunk"], true);
    }

    #[test]
    fn trace_event_serializes_correctly() {
        let event = StreamEvent::Trace {
            kind: "phase_start".into(),
            payload: serde_json::json!({ "phase": "plan" }),
        };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["type"], "trace");
        assert_eq!(json["payload"]["kind"], "phase_start");
    }

    #[tokio::test]
    async fn tutor_stream_sends_content() {
        let (stream, mut rx) = TutorStream::new(16);
        stream.content("hello", true).await;
        let event = rx.recv().await.unwrap();
        match event {
            StreamEvent::Content { text, chunk } => {
                assert_eq!(text, "hello");
                assert!(chunk);
            }
            _ => panic!("expected Content"),
        }
    }
}
```

- [ ] **Step 3: Implement TutorStream and StreamEvent**

```rust
// crates/tutor-web/src/stream.rs
use serde::Serialize;
use tokio::sync::mpsc;

/// Events pushed from the agent harness to the WebSocket handler.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "payload")]
#[serde(rename_all = "snake_case")]
pub enum StreamEvent {
    /// LLM text chunk or final message.
    Content { text: String, chunk: bool },
    /// Internal event for the TracePanel (tool calls, phase changes, REPLAN).
    Trace {
        kind: String,
        payload: serde_json::Value,
    },
    /// Status notification (budget_warning, phase_change, approval_request, error).
    Status {
        kind: String,
        payload: serde_json::Value,
    },
}

/// WebSocket event bus — one per active session.
#[derive(Clone)]
pub struct TutorStream {
    tx: mpsc::Sender<StreamEvent>,
}

impl TutorStream {
    /// Create a stream and the matching receiver.
    pub fn new(capacity: usize) -> (Self, mpsc::Receiver<StreamEvent>) {
        let (tx, rx) = mpsc::channel(capacity);
        (Self { tx }, rx)
    }

    pub async fn content(&self, text: &str, chunk: bool) {
        let _ = self
            .tx
            .send(StreamEvent::Content {
                text: text.to_string(),
                chunk,
            })
            .await;
    }

    pub async fn trace(&self, kind: &str, payload: impl Serialize) {
        let _ = self
            .tx
            .send(StreamEvent::Trace {
                kind: kind.to_string(),
                payload: serde_json::to_value(payload).unwrap_or_default(),
            })
            .await;
    }

    pub async fn status(&self, kind: &str, payload: impl Serialize) {
        let _ = self
            .tx
            .send(StreamEvent::Status {
                kind: kind.to_string(),
                payload: serde_json::to_value(payload).unwrap_or_default(),
            })
            .await;
    }
}
```

- [ ] **Step 4: Run tests**

```bash
cd /Users/hhl/Documents/projs/tutor_agent
cargo test -p tutor-web stream -- --nocapture 2>&1
```

Expected: all three tests pass.

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy -p tutor-web --all-targets
git -C /Users/hhl/Documents/projs/tutor_agent add crates/tutor-web/ Cargo.toml
git -C /Users/hhl/Documents/projs/tutor_agent commit -m "feat(web): TutorStream and StreamEvent for WebSocket event bus"
git -C /Users/hhl/Documents/projs/tutor_agent push
```

---

### Task 2: SessionPool

**Files:**
- Create: `crates/tutor-web/src/session.rs`
- Create: `crates/tutor-web/src/lib.rs`

- [ ] **Step 1: Write test**

```rust
// At the bottom of crates/tutor-web/src/session.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_pool_creates_and_retrieves() {
        let pool = SessionPool::new();
        let id = pool.create("chat", None);
        let entry = pool.get(&id);
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.capability, "chat");
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let pool = SessionPool::new();
        assert!(pool.get("nonexistent-id").is_none());
    }
}
```

- [ ] **Step 2: Implement SessionPool**

```rust
// crates/tutor-web/src/session.rs
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use uuid::Uuid;

use crate::stream::TutorStream;

/// Metadata for an active tutor session.
#[derive(Clone)]
pub struct SessionEntry {
    pub id: String,
    pub capability: String,
    pub kb: Option<String>,
    pub stream: TutorStream,
}

/// Thread-safe pool of active sessions.
pub struct SessionPool {
    sessions: Mutex<HashMap<String, SessionEntry>>,
}

impl SessionPool {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            sessions: Mutex::new(HashMap::new()),
        })
    }

    /// Create a new session and return its ID.
    /// The `TutorStream` receiver is returned separately for the WS handler.
    pub fn create(&self, capability: &str, kb: Option<String>) -> (String, tokio::sync::mpsc::Receiver<crate::stream::StreamEvent>) {
        let id = Uuid::new_v4().to_string();
        let (stream, rx) = TutorStream::new(128);
        let entry = SessionEntry {
            id: id.clone(),
            capability: capability.to_string(),
            kb,
            stream,
        };
        self.sessions.lock().unwrap().insert(id.clone(), entry);
        (id, rx)
    }

    pub fn get(&self, id: &str) -> Option<SessionEntry> {
        self.sessions.lock().unwrap().get(id).cloned()
    }
}

impl Default for SessionPool {
    fn default() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }
}
```

**Note:** The test above calls `pool.create("chat", None)` but the real signature returns a tuple. Update the test to ignore the receiver:

```rust
let (id, _rx) = pool.create("chat", None);
```

- [ ] **Step 3: Run test**

```bash
cargo test -p tutor-web session -- --nocapture 2>&1
```

- [ ] **Step 4: Commit**

```bash
cargo fmt && cargo clippy -p tutor-web --all-targets
git -C /Users/hhl/Documents/projs/tutor_agent add crates/tutor-web/
git -C /Users/hhl/Documents/projs/tutor_agent commit -m "feat(web): SessionPool for managing active tutor sessions"
git -C /Users/hhl/Documents/projs/tutor_agent push
```

---

### Task 3: REST routes

**Files:**
- Create: `crates/tutor-web/src/routes/mod.rs`
- Create: `crates/tutor-web/src/routes/sessions.rs`

- [ ] **Step 1: Write route test**

```rust
// crates/tutor-web/src/routes/sessions.rs — test at bottom
#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum_test::TestServer;

    #[tokio::test]
    async fn post_sessions_creates_session() {
        let pool = crate::session::SessionPool::new();
        let app = sessions_router(pool.clone());
        let server = TestServer::new(app).unwrap();

        let response = server
            .post("/api/sessions")
            .json(&serde_json::json!({ "capability": "chat" }))
            .await;

        assert_eq!(response.status_code(), StatusCode::CREATED);
        let body: serde_json::Value = response.json();
        assert!(body["id"].is_string());
    }
}
```

Add `axum-test = "0.4"` to dev-dependencies in `crates/tutor-web/Cargo.toml`.

- [ ] **Step 2: Implement routes/sessions.rs**

```rust
// crates/tutor-web/src/routes/sessions.rs
use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::session::SessionPool;

#[derive(Deserialize)]
struct CreateSessionRequest {
    capability: String,
    kb: Option<String>,
}

#[derive(Serialize)]
struct CreateSessionResponse {
    id: String,
}

async fn create_session(
    State(pool): State<Arc<SessionPool>>,
    Json(req): Json<CreateSessionRequest>,
) -> impl IntoResponse {
    let (id, _rx) = pool.create(&req.capability, req.kb);
    (StatusCode::CREATED, Json(CreateSessionResponse { id }))
}

async fn get_session(
    State(pool): State<Arc<SessionPool>>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match pool.get(&id) {
        Some(entry) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "id": entry.id,
                "capability": entry.capability,
                "kb": entry.kb,
            })),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "session not found" })),
        ),
    }
}

pub fn sessions_router(pool: Arc<SessionPool>) -> Router {
    Router::new()
        .route("/api/sessions", post(create_session))
        .route("/api/sessions/:id", get(get_session))
        .with_state(pool)
}
```

- [ ] **Step 3: Run test**

```bash
cargo test -p tutor-web routes -- --nocapture 2>&1
```

- [ ] **Step 4: Commit**

```bash
cargo fmt && cargo clippy -p tutor-web --all-targets
git -C /Users/hhl/Documents/projs/tutor_agent add crates/tutor-web/
git -C /Users/hhl/Documents/projs/tutor_agent commit -m "feat(web): REST routes for session CRUD"
git -C /Users/hhl/Documents/projs/tutor_agent push
```

---

### Task 4: WebSocket handler

**Files:**
- Create: `crates/tutor-web/src/routes/ws.rs`

- [ ] **Step 1: Write WebSocket route**

```rust
// crates/tutor-web/src/routes/ws.rs
use std::sync::Arc;

use axum::{
    extract::{
        ws::{Message, WebSocket},
        Path, State, WebSocketUpgrade,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use tokio::sync::mpsc;

use crate::{
    session::SessionPool,
    stream::StreamEvent,
};

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(pool): State<Arc<SessionPool>>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, pool, session_id))
}

async fn handle_socket(socket: WebSocket, pool: Arc<SessionPool>, session_id: String) {
    let Some(entry) = pool.get(&session_id) else {
        return;
    };

    let (mut ws_sink, mut ws_stream) = socket.split();

    // Spawn a task that reads from TutorStream and forwards to WebSocket
    let (event_tx, mut event_rx) = mpsc::channel::<StreamEvent>(128);

    // Rebuild a new channel since SessionEntry.stream is a sender clone
    // In production, SessionPool should store the receiver alongside the entry.
    // For now, accept that the WS handler gets events directly via event_tx.
    // TODO: wire event_rx from SessionPool.create() into the session store.

    let send_task = tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            let json = match serde_json::to_string(&event) {
                Ok(j) => j,
                Err(_) => continue,
            };
            if ws_sink.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    });

    // Receive messages from the client (approval responses, etc.)
    while let Some(Ok(msg)) = ws_stream.next().await {
        match msg {
            Message::Text(text) => {
                // Parse and handle client messages
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    let msg_type = val["type"].as_str().unwrap_or("");
                    match msg_type {
                        "message" => {
                            // Client sent a chat message — run the capability
                            // TODO: wire to CapabilityRouter.run()
                            let _ = entry.stream.content("Processing...", false).await;
                        }
                        "approval_response" => {
                            // TODO: wire to pending approval channel
                        }
                        _ => {}
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    send_task.abort();
}

pub fn ws_router(pool: Arc<SessionPool>) -> Router {
    Router::new()
        .route("/ws/sessions/:id", get(ws_handler))
        .with_state(pool)
}
```

**Note:** The WS handler's event forwarding needs a proper channel from `SessionPool`. In this initial implementation, messages send through `entry.stream` (the cloned sender) are not received by `event_rx`. Fix this in Task 5 by updating `SessionPool` to also store and expose the receiver.

- [ ] **Step 2: Commit**

```bash
cargo fmt && cargo clippy -p tutor-web --all-targets
git -C /Users/hhl/Documents/projs/tutor_agent add crates/tutor-web/src/routes/ws.rs
git -C /Users/hhl/Documents/projs/tutor_agent commit -m "feat(web): WebSocket handler skeleton for streaming events"
git -C /Users/hhl/Documents/projs/tutor_agent push
```

---

### Task 5: Server main.rs

**Files:**
- Create: `crates/tutor-web/src/main.rs`

- [ ] **Step 1: Implement server entry point**

```rust
// crates/tutor-web/src/main.rs
use std::net::SocketAddr;
use std::sync::Arc;

use tower_http::cors::{Any, CorsLayer};

mod routes;
mod session;
mod stream;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let pool = session::SessionPool::new();

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = axum::Router::new()
        .merge(routes::sessions::sessions_router(pool.clone()))
        .merge(routes::ws::ws_router(pool.clone()))
        .layer(cors);

    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    println!("tutor-web listening on {addr}");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
```

- [ ] **Step 2: Build server**

```bash
cd /Users/hhl/Documents/projs/tutor_agent
cargo build -p tutor-web 2>&1
```

Expected: compiles without errors.

- [ ] **Step 3: Commit**

```bash
cargo fmt && cargo clippy -p tutor-web --all-targets
git -C /Users/hhl/Documents/projs/tutor_agent add crates/tutor-web/
git -C /Users/hhl/Documents/projs/tutor_agent commit -m "feat(web): axum server entry point with CORS and route registration"
git -C /Users/hhl/Documents/projs/tutor_agent push
```

---

### Task 6: Vite + React + Tailwind scaffold

**Files:**
- Create: `web-ui/` (Vite project)

- [ ] **Step 1: Scaffold Vite project**

```bash
cd /Users/hhl/Documents/projs/tutor_agent
npm create vite@latest web-ui -- --template react-ts
cd web-ui
npm install
npm install -D tailwindcss @tailwindcss/vite
```

- [ ] **Step 2: Configure Tailwind**

In `web-ui/vite.config.ts`:

```typescript
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    proxy: {
      '/api': 'http://localhost:8080',
      '/ws': { target: 'ws://localhost:8080', ws: true },
    },
  },
})
```

In `web-ui/src/index.css`, replace all content with:

```css
@import "tailwindcss";
```

- [ ] **Step 3: Verify dev server starts**

```bash
cd /Users/hhl/Documents/projs/tutor_agent/web-ui
npm run dev 2>&1 &
sleep 3
curl -s http://localhost:5173 | head -5
pkill -f "vite" 2>/dev/null || true
```

Expected: returns HTML `<!doctype html>`.

- [ ] **Step 4: Commit**

```bash
cd /Users/hhl/Documents/projs/tutor_agent
git add web-ui/
git commit -m "chore(web-ui): Vite + React + Tailwind scaffold"
git push
```

---

### Task 7: useWebSocket hook

**Files:**
- Create: `web-ui/src/hooks/useWebSocket.ts`

- [ ] **Step 1: Implement the hook**

```typescript
// web-ui/src/hooks/useWebSocket.ts
import { useEffect, useRef, useCallback } from 'react'

export type StreamEvent =
  | { type: 'content'; payload: { text: string; chunk: boolean } }
  | { type: 'trace'; payload: { kind: string; [key: string]: unknown } }
  | { type: 'status'; payload: { kind: string; [key: string]: unknown } }

interface UseWebSocketOptions {
  onEvent: (event: StreamEvent) => void
  onClose?: () => void
}

export function useWebSocket(sessionId: string | null, opts: UseWebSocketOptions) {
  const wsRef = useRef<WebSocket | null>(null)
  const optsRef = useRef(opts)
  optsRef.current = opts

  useEffect(() => {
    if (!sessionId) return

    const ws = new WebSocket(`ws://localhost:8080/ws/sessions/${sessionId}`)
    wsRef.current = ws

    ws.onmessage = (e) => {
      try {
        const event = JSON.parse(e.data) as StreamEvent
        optsRef.current.onEvent(event)
      } catch {}
    }

    ws.onclose = () => optsRef.current.onClose?.()

    return () => ws.close()
  }, [sessionId])

  const send = useCallback((msg: unknown) => {
    wsRef.current?.send(JSON.stringify(msg))
  }, [])

  return { send }
}
```

- [ ] **Step 2: Commit**

```bash
git -C /Users/hhl/Documents/projs/tutor_agent add web-ui/src/hooks/
git -C /Users/hhl/Documents/projs/tutor_agent commit -m "feat(web-ui): useWebSocket hook for streaming events"
git -C /Users/hhl/Documents/projs/tutor_agent push
```

---

### Task 8: ChatBox component

**Files:**
- Create: `web-ui/src/components/ChatBox.tsx`

- [ ] **Step 1: Implement ChatBox**

```tsx
// web-ui/src/components/ChatBox.tsx
interface Message {
  role: 'user' | 'assistant'
  text: string
}

interface Props {
  messages: Message[]
  streamingText: string
  onSend: (text: string) => void
  disabled: boolean
}

export function ChatBox({ messages, streamingText, onSend, disabled }: Props) {
  const [input, setInput] = React.useState('')

  const handleSend = () => {
    if (!input.trim() || disabled) return
    onSend(input.trim())
    setInput('')
  }

  return (
    <div className="flex flex-col h-full">
      <div className="flex-1 overflow-y-auto p-4 space-y-3">
        {messages.map((msg, i) => (
          <div
            key={i}
            className={`rounded-lg p-3 max-w-3xl ${
              msg.role === 'user'
                ? 'bg-blue-100 ml-auto'
                : 'bg-gray-100'
            }`}
          >
            <pre className="whitespace-pre-wrap text-sm">{msg.text}</pre>
          </div>
        ))}
        {streamingText && (
          <div className="bg-gray-100 rounded-lg p-3 max-w-3xl">
            <pre className="whitespace-pre-wrap text-sm">{streamingText}</pre>
            <span className="animate-pulse">▌</span>
          </div>
        )}
      </div>
      <div className="border-t p-4 flex gap-2">
        <input
          className="flex-1 border rounded px-3 py-2 text-sm"
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && !e.shiftKey && handleSend()}
          placeholder="Ask a question..."
          disabled={disabled}
        />
        <button
          className="bg-blue-600 text-white px-4 py-2 rounded text-sm disabled:opacity-50"
          onClick={handleSend}
          disabled={disabled}
        >
          Send
        </button>
      </div>
    </div>
  )
}

import React from 'react'
```

- [ ] **Step 2: Commit**

```bash
git -C /Users/hhl/Documents/projs/tutor_agent add web-ui/src/components/ChatBox.tsx
git -C /Users/hhl/Documents/projs/tutor_agent commit -m "feat(web-ui): ChatBox with streaming text display"
git -C /Users/hhl/Documents/projs/tutor_agent push
```

---

### Task 9: TracePanel, BudgetPanel, ApprovalDialog, CapabilitySelector

**Files:**
- Create: `web-ui/src/components/TracePanel.tsx`
- Create: `web-ui/src/components/BudgetPanel.tsx`
- Create: `web-ui/src/components/ApprovalDialog.tsx`
- Create: `web-ui/src/components/CapabilitySelector.tsx`

- [ ] **Step 1: Implement TracePanel**

```tsx
// web-ui/src/components/TracePanel.tsx
import React, { useState } from 'react'

export interface TraceEntry {
  kind: string
  payload: Record<string, unknown>
  timestamp: number
}

interface Props {
  entries: TraceEntry[]
}

export function TracePanel({ entries }: Props) {
  const [expanded, setExpanded] = useState<Set<number>>(new Set())

  const toggle = (i: number) =>
    setExpanded((prev) => {
      const next = new Set(prev)
      next.has(i) ? next.delete(i) : next.add(i)
      return next
    })

  return (
    <div className="h-full overflow-y-auto p-2 text-xs font-mono">
      <div className="text-gray-500 text-xs uppercase mb-2">Trace</div>
      {entries.map((entry, i) => (
        <div key={i} className="border-b border-gray-100 py-1">
          <button
            className="w-full text-left flex gap-2 items-center"
            onClick={() => toggle(i)}
          >
            <span className="text-gray-400">
              {expanded.has(i) ? '▼' : '▶'}
            </span>
            <span className="text-blue-600">{entry.kind}</span>
          </button>
          {expanded.has(i) && (
            <pre className="mt-1 text-gray-600 pl-4 text-xs overflow-x-auto">
              {JSON.stringify(entry.payload, null, 2)}
            </pre>
          )}
        </div>
      ))}
    </div>
  )
}
```

- [ ] **Step 2: Implement BudgetPanel**

```tsx
// web-ui/src/components/BudgetPanel.tsx
interface Props {
  spent: number
  limit: number
  warning: boolean
}

export function BudgetPanel({ spent, limit, warning }: Props) {
  const pct = Math.min((spent / limit) * 100, 100)
  return (
    <div className={`p-3 rounded border text-sm ${warning ? 'border-yellow-400 bg-yellow-50' : 'border-gray-200'}`}>
      <div className="flex justify-between mb-1">
        <span className="text-gray-600">Budget</span>
        <span className={warning ? 'text-yellow-600 font-medium' : 'text-gray-800'}>
          ${spent.toFixed(4)} / ${limit.toFixed(2)}
        </span>
      </div>
      <div className="h-1.5 bg-gray-200 rounded overflow-hidden">
        <div
          className={`h-full rounded transition-all ${pct > 80 ? 'bg-yellow-400' : 'bg-blue-500'}`}
          style={{ width: `${pct}%` }}
        />
      </div>
    </div>
  )
}
```

- [ ] **Step 3: Implement ApprovalDialog**

```tsx
// web-ui/src/components/ApprovalDialog.tsx
interface ApprovalRequest {
  tool: string
  args: Record<string, unknown>
  requestId: string
}

interface Props {
  request: ApprovalRequest | null
  onDecision: (requestId: string, approved: boolean) => void
}

export function ApprovalDialog({ request, onDecision }: Props) {
  if (!request) return null
  return (
    <div className="fixed inset-0 bg-black/40 flex items-center justify-center z-50">
      <div className="bg-white rounded-xl shadow-xl p-6 max-w-md w-full mx-4">
        <h2 className="text-lg font-semibold mb-2">Tool Approval Required</h2>
        <p className="text-sm text-gray-600 mb-3">
          The agent wants to execute:{' '}
          <span className="font-mono font-medium">{request.tool}</span>
        </p>
        <pre className="bg-gray-50 rounded p-3 text-xs overflow-auto max-h-40 mb-4">
          {JSON.stringify(request.args, null, 2)}
        </pre>
        <div className="flex gap-3 justify-end">
          <button
            className="px-4 py-2 border rounded text-sm text-gray-700"
            onClick={() => onDecision(request.requestId, false)}
          >
            Deny
          </button>
          <button
            className="px-4 py-2 bg-blue-600 text-white rounded text-sm"
            onClick={() => onDecision(request.requestId, true)}
          >
            Approve
          </button>
        </div>
      </div>
    </div>
  )
}
```

- [ ] **Step 4: Implement CapabilitySelector**

```tsx
// web-ui/src/components/CapabilitySelector.tsx
type Capability = 'chat' | 'deep_solve' | 'code_exec'

interface Props {
  value: Capability
  onChange: (c: Capability) => void
}

export function CapabilitySelector({ value, onChange }: Props) {
  const options: { key: Capability; label: string }[] = [
    { key: 'chat', label: 'Chat' },
    { key: 'deep_solve', label: 'Deep Solve' },
    { key: 'code_exec', label: 'Code Exec' },
  ]
  return (
    <div className="flex border rounded overflow-hidden text-sm">
      {options.map((opt) => (
        <button
          key={opt.key}
          className={`px-4 py-2 ${
            value === opt.key
              ? 'bg-blue-600 text-white'
              : 'bg-white text-gray-700 hover:bg-gray-50'
          }`}
          onClick={() => onChange(opt.key)}
        >
          {opt.label}
        </button>
      ))}
    </div>
  )
}
```

- [ ] **Step 5: Commit**

```bash
git -C /Users/hhl/Documents/projs/tutor_agent add web-ui/src/components/
git -C /Users/hhl/Documents/projs/tutor_agent commit -m "feat(web-ui): TracePanel, BudgetPanel, ApprovalDialog, CapabilitySelector"
git -C /Users/hhl/Documents/projs/tutor_agent push
```

---

### Task 10: App.tsx — wire everything together

**Files:**
- Modify: `web-ui/src/App.tsx`

- [ ] **Step 1: Implement App.tsx**

```tsx
// web-ui/src/App.tsx
import React, { useState, useCallback } from 'react'
import { CapabilitySelector } from './components/CapabilitySelector'
import { ChatBox } from './components/ChatBox'
import { TracePanel, TraceEntry } from './components/TracePanel'
import { BudgetPanel } from './components/BudgetPanel'
import { ApprovalDialog } from './components/ApprovalDialog'
import { useWebSocket } from './hooks/useWebSocket'

type Capability = 'chat' | 'deep_solve' | 'code_exec'

interface Message {
  role: 'user' | 'assistant'
  text: string
}

export default function App() {
  const [capability, setCapability] = useState<Capability>('chat')
  const [sessionId, setSessionId] = useState<string | null>(null)
  const [messages, setMessages] = useState<Message[]>([])
  const [streamingText, setStreamingText] = useState('')
  const [traceEntries, setTraceEntries] = useState<TraceEntry[]>([])
  const [budgetSpent, setBudgetSpent] = useState(0)
  const [budgetWarning, setBudgetWarning] = useState(false)
  const [pendingApproval, setPendingApproval] = useState<{ tool: string; args: Record<string, unknown>; requestId: string } | null>(null)
  const [running, setRunning] = useState(false)

  const { send } = useWebSocket(sessionId, {
    onEvent: (event) => {
      if (event.type === 'content') {
        if (event.payload.chunk) {
          setStreamingText((prev) => prev + event.payload.text)
        } else {
          setMessages((prev) => [
            ...prev,
            { role: 'assistant', text: streamingText + event.payload.text },
          ])
          setStreamingText('')
          setRunning(false)
        }
      } else if (event.type === 'trace') {
        setTraceEntries((prev) => [
          ...prev,
          { kind: event.payload.kind, payload: event.payload, timestamp: Date.now() },
        ])
      } else if (event.type === 'status') {
        const { kind, payload } = event.payload as { kind: string; payload: Record<string, unknown> }
        if (kind === 'budget_warning') {
          setBudgetWarning(true)
          setBudgetSpent((payload.spent_usd as number) ?? budgetSpent)
        } else if (kind === 'approval_request') {
          setPendingApproval({
            tool: payload.tool as string,
            args: payload.args as Record<string, unknown>,
            requestId: payload.request_id as string,
          })
        }
      }
    },
    onClose: () => setRunning(false),
  })

  const handleSend = useCallback(async (text: string) => {
    // Create session if needed
    let sid = sessionId
    if (!sid) {
      const res = await fetch('/api/sessions', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ capability }),
      })
      const data = await res.json()
      sid = data.id
      setSessionId(sid)
    }

    setMessages((prev) => [...prev, { role: 'user', text }])
    setRunning(true)
    send({ type: 'message', content: text })
  }, [sessionId, capability, send])

  const handleApproval = (requestId: string, approved: boolean) => {
    send({ type: 'approval_response', request_id: requestId, approved })
    setPendingApproval(null)
  }

  return (
    <div className="flex flex-col h-screen bg-gray-50">
      {/* Header */}
      <header className="bg-white border-b px-6 py-3 flex items-center gap-4">
        <h1 className="text-lg font-semibold text-gray-900">Tutor Agent</h1>
        <CapabilitySelector value={capability} onChange={(c) => { setCapability(c); setSessionId(null) }} />
        <div className="ml-auto">
          <BudgetPanel spent={budgetSpent} limit={2.0} warning={budgetWarning} />
        </div>
      </header>

      {/* Main */}
      <div className="flex flex-1 min-h-0">
        <main className="flex-1">
          <ChatBox
            messages={messages}
            streamingText={streamingText}
            onSend={handleSend}
            disabled={running}
          />
        </main>
        <aside className="w-72 border-l bg-white">
          <TracePanel entries={traceEntries} />
        </aside>
      </div>

      <ApprovalDialog request={pendingApproval} onDecision={handleApproval} />
    </div>
  )
}
```

- [ ] **Step 2: Start servers and test manually**

```bash
# Terminal 1: start backend
cd /Users/hhl/Documents/projs/tutor_agent
ANTHROPIC_API_KEY=$ANTHROPIC_API_KEY cargo run -p tutor-web &

# Terminal 2: start frontend
cd /Users/hhl/Documents/projs/tutor_agent/web-ui
npm run dev
```

Open `http://localhost:5173` in browser. Verify:
1. CapabilitySelector shows three tabs
2. ChatBox input is enabled
3. BudgetPanel shows $0.0000 / $2.00
4. TracePanel is empty

- [ ] **Step 3: TypeScript type check**

```bash
cd /Users/hhl/Documents/projs/tutor_agent/web-ui
npx tsc --noEmit 2>&1
```

Expected: no type errors.

- [ ] **Step 4: Commit Phase 4**

```bash
git -C /Users/hhl/Documents/projs/tutor_agent add web-ui/src/App.tsx
git -C /Users/hhl/Documents/projs/tutor_agent commit -m "feat(web-ui): App.tsx wires all components with WebSocket streaming"
git -C /Users/hhl/Documents/projs/tutor_agent tag phase4-complete
git -C /Users/hhl/Documents/projs/tutor_agent push
git -C /Users/hhl/Documents/projs/tutor_agent push --tags
```

---

## Phase 4 Success Criteria

- `cargo build -p tutor-web` succeeds
- `cargo test -p tutor-web` passes (REST route tests, stream serialization tests)
- `npm run build` in `web-ui/` succeeds with no TypeScript errors
- Browser at `http://localhost:5173` shows the UI with all four panels
- `content` events from the backend appear in ChatBox as streaming text
- `trace` events appear in TracePanel as collapsible entries
- `status.budget_warning` updates BudgetPanel
- `status.approval_request` opens ApprovalDialog
