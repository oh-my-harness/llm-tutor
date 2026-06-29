# Tauri Desktop Release Plan

> Status: active planning | Date: 2026-06-28 | Scope: build the first usable
> desktop release of `llm-tutor` with Tauri, bundled React UI, and a managed
> `tutor-web` backend sidecar.

## 1. Goal

The first desktop release should let a user install and start `llm-tutor` like
a normal local application.

The user should not need to:

- run `cargo run -p tutor-web` manually,
- run `npm run dev` manually,
- open a browser,
- understand which port the backend uses,
- keep project checkout paths as the data directory.

The desktop app should provide:

- a native window,
- the existing React UI,
- a locally managed Rust backend,
- persistent local data,
- usable LLM / embedding / search settings,
- a repeatable release build path.

## 2. Architecture Decision

Use **Tauri + sidecar backend** for the first release.

```text
llm-tutor desktop app
  -> Tauri shell
      -> loads web-ui/dist
      -> starts tutor-web sidecar
      -> passes host, port, and data directory to tutor-web
      -> provides backend URL to the frontend
  -> tutor-web sidecar
      -> serves REST API on 127.0.0.1:<port>
      -> serves WebSocket on 127.0.0.1:<port>
      -> reads/writes local app data
      -> runs agent, tools, RAG, quiz, memory, notebook, books
```

Do not rewrite `tutor-web` as Tauri commands for v0.1. The existing Axum
backend already owns streaming, sessions, uploads, RAG, tools, and product
storage. Rewriting those as Tauri commands would add risk without improving the
first release.

## 3. Release Scope

### In Scope

- Windows desktop release first.
- Tauri project added to the repository.
- Existing `web-ui` bundled into the desktop app.
- `tutor-web` built as a sidecar binary.
- Tauri starts and stops the sidecar.
- Backend listens only on `127.0.0.1`.
- Frontend can discover the backend base URL in desktop mode.
- Desktop data directory is separate from development `.llm-tutor/`.
- A release script builds frontend, backend, and Tauri bundle.
- README documents desktop usage.

### Out of Scope

- Cloud sync.
- Multi-user accounts.
- Built-in model service.
- Auto update.
- System keychain storage for API keys.
- Rewriting all backend routes as Tauri commands.
- macOS / Linux release packaging.

## 4. Data Directory

Development mode can keep using the repository-local `.llm-tutor/` directory.

Desktop release should use the OS app data directory, for example on Windows:

```text
%APPDATA%\llm-tutor
```

The sidecar should receive this directory through either:

- environment variable: `LLM_TUTOR_HOME`,
- or CLI argument: `--data-dir <path>`.

Preferred v0.1 implementation:

```text
Tauri app starts
  -> resolve app_data_dir()
  -> create llm-tutor data directory if missing
  -> spawn tutor-web with LLM_TUTOR_HOME=<app data dir>
```

## 5. Port Strategy

The backend must not listen on `0.0.0.0`.

Use:

```text
127.0.0.1:<dynamic port>
```

Recommended v0.1 implementation:

1. Tauri finds a free TCP port.
2. Tauri starts `tutor-web` with `--host 127.0.0.1 --port <port>`.
3. Tauri stores the backend URL in app state.
4. Frontend asks Tauri for the backend URL.
5. Frontend builds REST and WebSocket URLs from that base URL.

Fallback if dynamic port is too slow to implement:

- use fixed `127.0.0.1:8080`,
- detect conflict,
- show a clear startup error.

Dynamic port is preferred for the first real release.

## 6. Frontend API Adaptation

The current web UI relies on Vite proxy in development:

```text
/api -> http://localhost:8080
/ws  -> ws://localhost:8080
```

Desktop mode has no Vite proxy, so frontend code should use a small API client:

```text
apiFetch("/api/sessions")
apiUrl("/api/knowledge-bases")
wsUrl("/ws/session/<id>")
```

Behavior:

- Browser/dev mode: keep relative URLs.
- Tauri/desktop mode: use backend URL returned by Tauri command.

Do not scatter `http://127.0.0.1:<port>` construction across components.

## 7. Backend Changes

`tutor-web` should support runtime configuration:

```text
tutor-web --host 127.0.0.1 --port 43127 --data-dir <path>
```

If CLI arguments are not provided:

- host defaults to `127.0.0.1`,
- port defaults to `8080`,
- data dir defaults to current existing behavior.

Required backend tasks:

- add host/port/data-dir config parsing,
- route all product stores through the configured data root,
- ensure LanceDB/RAG root also lives under the configured data root,
- ensure logs never print API keys,
- return clear startup errors.

## 8. Tauri App Structure

Add:

```text
src-tauri/
  Cargo.toml
  tauri.conf.json
  src/main.rs
  icons/
```

Tauri responsibilities:

- create native window,
- load `web-ui/dist`,
- spawn `tutor-web` sidecar,
- stop sidecar on app exit,
- expose `get_backend_url` command,
- expose `open_data_dir` command later,
- expose backend health state later.

## 9. Build and Packaging

Recommended commands:

```powershell
# build frontend
cd web-ui
npm run build
cd ..

# build backend sidecar
cargo build --release -p tutor-web

# build desktop bundle
cargo tauri build
```

Add a single release script:

```text
scripts/build-desktop.ps1
```

The script should:

1. run frontend build,
2. run backend release build,
3. copy or let Tauri bundle the sidecar,
4. run Tauri bundle,
5. print output artifact paths.

First Windows artifacts:

- portable `.exe` or zipped app folder for quick testing,
- installer if Tauri bundler setup is stable.

## 10. Implementation Phases

### Phase 1: Desktop Skeleton

Status: in progress.

Tasks:

- [x] Add Tauri project.
- [x] Configure dev URL to existing Vite dev server.
- [x] Configure production dist path to `web-ui/dist`.
- [x] Add basic app window title, icon placeholder, and app metadata.
- [ ] Verify desktop window can load existing UI.

Acceptance:

- `cargo tauri dev` opens the current UI.
- No backend sidecar is required yet in this phase.

### Phase 2: Backend Sidecar

Status: planned.

Tasks:

- [ ] Add `tutor-web` host/port/data-dir runtime config.
- [ ] Add sidecar declaration to Tauri config.
- [ ] Spawn `tutor-web` on app startup.
- [ ] Kill `tutor-web` on app exit.
- [ ] Implement dynamic local port selection.
- [ ] Implement Tauri command: `get_backend_url`.
- [ ] Add frontend API URL resolver.
- [ ] Update REST fetches and WebSocket creation to use resolver.

Acceptance:

- Desktop app starts backend automatically.
- Chat session list can load from sidecar.
- WebSocket chat can connect from the desktop app.
- Closing the desktop app stops the sidecar.

### Phase 3: Local Data Directory

Status: planned.

Tasks:

- [ ] Resolve Tauri app data directory.
- [ ] Pass app data directory to sidecar.
- [ ] Move sessions, settings, knowledge bases, quizzes, memory, notebooks,
      books, uploaded documents, and LanceDB under the configured data root.
- [ ] Add settings/status UI display for current data directory.
- [ ] Add "open data directory" desktop command if practical.

Acceptance:

- Desktop data survives app restart.
- Desktop data does not require running inside the repository.
- Knowledge base upload and RAG search work after restart.

### Phase 4: Release Build Script

Status: planned.

Tasks:

- [ ] Add `scripts/build-desktop.ps1`.
- [ ] Add root-level documentation for desktop build prerequisites.
- [ ] Build `web-ui`.
- [ ] Build `tutor-web --release`.
- [ ] Build Tauri bundle.
- [ ] Restore or ignore generated build cache files.

Acceptance:

- A developer can produce a desktop artifact with one documented command.
- Build output path is printed clearly.

### Phase 5: First Release QA

Status: planned.

Tasks:

- [ ] Install or unpack app on a clean Windows machine/profile.
- [ ] Start app with no existing data.
- [ ] Configure one LLM provider.
- [ ] Send a chat message and receive streaming response.
- [ ] Configure embedding provider.
- [ ] Create knowledge base and upload text/PDF.
- [ ] Ask a RAG question and verify citation links.
- [ ] Generate a Quiz from conversation or knowledge base.
- [ ] Save a Research report to Notebook.
- [ ] Restart app and verify data persistence.

Acceptance:

- App can be used end to end without terminal commands.
- Startup failures show understandable messages.
- No API keys appear in logs or trace.

## 11. Risks and Mitigations

| Risk | Impact | Mitigation |
| --- | --- | --- |
| Sidecar does not stop | Leaves background process running | Track child process and kill on exit |
| Port conflict | App cannot connect to backend | Use dynamic port |
| Frontend URL assumptions | API/WS fail in desktop mode | Centralize URL construction |
| Data directory mismatch | User data appears lost | Explicit data root and settings display |
| API keys in local files | Security concern | Keep first version local-only; avoid logs; plan keychain later |
| Tauri bundling sidecar complexity | Release build blocked | Start with portable/debug bundle if installer takes longer |
| Windows code signing absent | Installer trust warnings | Accept for internal v0.1; plan signing before public release |

## 12. Done Criteria for v0.1 Desktop Release

- A Windows desktop artifact exists.
- User can launch the app without terminal commands.
- Backend sidecar starts automatically.
- Frontend talks to the sidecar over local REST/WebSocket.
- LLM settings can be configured in the app.
- Chat works with streaming output.
- Knowledge base upload and RAG search work.
- Local data persists across restart.
- Build instructions are documented.

## 13. Suggested First Implementation Order

1. Add Tauri skeleton and load existing `web-ui`.
2. Add backend host/port/data-dir args.
3. Add sidecar spawn with fixed port.
4. Replace fixed port with dynamic port.
5. Add frontend API URL resolver.
6. Move desktop data root to app data dir.
7. Add build script.
8. Run QA checklist.
