# Tauri Desktop Release Plan

> Status: implementation mostly complete; manual desktop QA pending | Date:
> 2026-06-28 | Scope: build the first usable desktop release of `llm-tutor`
> with Tauri, bundled React UI, and a managed `tutor-web` backend sidecar.

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
- macOS desktop artifacts for every future public desktop release.
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
- Linux release packaging.

## 4. Data Directory

Development mode can keep using the repository-local `.llm-tutor/` directory.

Desktop release should use the OS app data directory, for example on Windows:

```text
%APPDATA%\llm-tutor
```

The backend supports receiving this directory through either:

- environment variable: `LLM_TUTOR_HOME`,
- or CLI argument: `--data-dir <path>`.

Current v0.1 implementation:

```text
Tauri app starts
  -> resolve app_data_dir()
  -> create llm-tutor data directory if missing
  -> spawn tutor-web with --data-dir <app data dir>
```

## 5. Port Strategy

The backend must not listen on `0.0.0.0`.

Use:

```text
127.0.0.1:<dynamic port>
```

Current v0.1 implementation:

1. Tauri finds a free TCP port.
2. Tauri starts `tutor-web` with `--host 127.0.0.1 --port <port>`.
3. Tauri stores the backend URL in app state.
4. Frontend asks Tauri for the backend URL.
5. Frontend builds REST and WebSocket URLs from that base URL.

Fallback design if dynamic port ever regresses:

- use fixed `127.0.0.1:8080`,
- detect conflict,
- show a clear startup error.

Dynamic port selection is implemented for both desktop development and release
startup.

## 6. Frontend API Adaptation

The current web UI relies on Vite proxy in development:

```text
/api -> http://localhost:8080
/ws  -> ws://localhost:8080
```

Desktop mode has no Vite proxy. The frontend now initializes a desktop API
bridge that asks Tauri for the backend base URL and rewrites `/api` fetch/XHR
calls to the sidecar. Code that builds explicit URLs should continue to use the
shared helpers:

```text
fetch(apiUrl("/api/sessions"))
apiUrl("/api/knowledge-bases")
wsUrl("/ws/session/<id>")
```

Current behavior:

- Browser/dev mode: keep relative URLs.
- Tauri/desktop mode: use backend URL returned by Tauri command.
- WebSocket URLs are built with `wsUrl(...)` from the same backend URL.
- Existing component-level `fetch("/api/...")` calls are patched in desktop
  mode, but new shared code should still prefer the API helpers.

Do not scatter `http://127.0.0.1:<port>` construction across components.

## 7. Backend Changes

`tutor-web` supports runtime configuration:

```text
tutor-web --host 127.0.0.1 --port 43127 --data-dir <path>
```

If CLI arguments are not provided:

- host defaults to `127.0.0.1`,
- port defaults to `8080`,
- data dir defaults to current existing behavior.

Backend tasks:

- [x] add host/port/data-dir config parsing,
- [x] support `LLM_TUTOR_HOME` as a data-root fallback,
- [x] route product stores through the configured data root,
- [x] ensure LanceDB/RAG root also lives under the configured data root,
- [x] add backend Settings Store at `<data-dir>/settings.json`,
- [x] avoid logging API keys in normal settings and startup flows,
- [x] return clear startup errors from CLI argument parsing and bind failures.

## 8. Tauri App Structure

Add:

```text
src-tauri/
  Cargo.toml
  tauri.conf.json
  tauri.release.conf.json
  src/main.rs
  capabilities/
  binaries/
  icons/
```

Tauri responsibilities:

- create native window,
- load `web-ui/dist`,
- spawn `tutor-web` sidecar,
- stop sidecar on app exit,
- expose `get_backend_url` command,
- expose `get_data_dir` command,
- expose `open_data_dir` command,
- expose backend health state later.

## 9. Build and Packaging

Low-level commands:

```powershell
# build frontend
cd web-ui
npm run build
cd ..

# build backend sidecar
cargo build --release -p tutor-web

# build desktop bundle with release sidecar config
cargo tauri build --config src-tauri/tauri.release.conf.json
```

The primary release script is:

```text
scripts/build-desktop.ps1
```

The script:

1. builds the backend release binary for the selected Rust target,
2. copies it to the Tauri v2 sidecar filename under `src-tauri/binaries/`,
3. runs `cargo tauri build --config src-tauri/tauri.release.conf.json`,
4. lets Tauri run the configured `beforeBuildCommand` for `web-ui`,
5. prints output artifact paths.

First Windows artifacts:

- portable `.exe` or zipped app folder for quick testing,
- installer if Tauri bundler setup is stable.

macOS artifacts:

- Build on macOS through GitHub Actions. The first CI target is
  `macos-13`/`x86_64-apple-darwin`; Apple Silicon or universal builds can be
  added later.
- Publish a `.dmg` for each public desktop release.
- Start with unsigned artifacts for internal validation if needed.
- Before broader public distribution, add Developer ID signing, notarization,
  and stapling.
- Upload macOS artifacts to the same GitHub Release as the Windows installers.

Release automation:

- `.github/workflows/release-desktop.yml` exists and is the preferred release
  path once CI secrets and dependency access are configured.
- Pushing a `v*` tag builds Windows and macOS desktop artifacts and uploads
  them to the matching GitHub Release.
- Manual `workflow_dispatch` builds the same artifacts as workflow artifacts
  without publishing a release, so it can be used to validate packaging changes.
- The workflow requires a repository secret named `PRIVATE_DEPS_TOKEN` when
  workspace dependencies are private. Use a fine-grained GitHub token or GitHub
  App token with read access to private dependency repositories such as
  `oh-my-harness/llm-api-adapter` and any private runtime repositories.
- Cargo is configured with `CARGO_NET_GIT_FETCH_WITH_CLI=true` so git dependency
  fetches use the token configured by the workflow instead of failing with an
  ambiguous `revision not found` error.
- Local `scripts/build-desktop.ps1` and `scripts/qa-desktop.ps1` remain the
  fallback path for debugging Windows release issues.

## 10. Implementation Phases

### Phase 1: Desktop Skeleton

Status: implementation complete; pending manual desktop QA.

Tasks:

- [x] Add Tauri project.
- [x] Configure dev URL to existing Vite dev server.
- [x] Configure production dist path to `web-ui/dist`.
- [x] Add basic app window title, icon placeholder, and app metadata.
- [x] Add desktop dev script that builds `tutor-web` and starts Vite.
- [ ] Verify desktop window can load existing UI.

Acceptance:

- `cargo tauri dev` starts the Vite dev server and opens the current UI.
- In debug desktop mode, Tauri starts a local `tutor-web` process via Cargo or
  an existing debug binary.

### Phase 2: Backend Sidecar

Status: implementation complete; pending manual desktop QA.

Tasks:

- [x] Add `tutor-web` host/port runtime config.
- [x] Add sidecar declaration to release Tauri config.
- [x] Spawn `tutor-web` on app startup.
- [x] Kill `tutor-web` on app exit.
- [x] Implement dynamic local port selection.
- [x] Implement Tauri command: `get_backend_url`.
- [x] Add frontend API URL resolver.
- [x] Update REST fetches and WebSocket creation to use resolver.

Acceptance:

- Desktop app starts backend automatically.
- Chat session list can load from sidecar.
- WebSocket chat can connect from the desktop app.
- Closing the desktop app stops the sidecar.

### Phase 3: Local Data Directory

Status: implementation complete; pending manual desktop QA.

Tasks:

- [x] Resolve Tauri app data directory.
- [x] Pass app data directory to sidecar with `--data-dir <path>`.
- [x] Move backend product stores under the configured data root: sessions,
      knowledge bases, quizzes, memory, notebooks, books, uploaded documents,
      and LanceDB/RAG data.
- [x] Add settings/status UI display for current data directory.
- [x] Add "open data directory" desktop command.
- [x] Decide whether frontend localStorage settings should remain in WebView
      storage for v0.1 or move into a backend settings store.

Decision:

- v0.1 stores LLM, embedding, search, budget, and approval settings in the
  backend Settings Store at `<data-dir>/settings.json`.
- The frontend still writes a localStorage compatibility cache, but startup
  prefers the backend Settings Store and migrates existing localStorage settings
  into the backend when the backend store is empty.
- Backend product data and settings now share the configured app data root.

Acceptance:

- Desktop data survives app restart.
- Desktop data does not require running inside the repository.
- Knowledge base upload and RAG search work after restart.

### Phase 4: Release Build Script

Status: implementation complete; pending manual release build QA.

Tasks:

- [x] Add `scripts/build-desktop.ps1`.
- [x] Add root-level documentation for desktop build prerequisites.
- [x] Build `web-ui`.
- [x] Build `tutor-web --release`.
- [x] Build Tauri bundle.
- [x] Restore or ignore generated build cache files.

Decision:

- `cargo tauri build` runs the configured `beforeBuildCommand`, so the release
  script lets Tauri build `web-ui` instead of running a duplicate frontend
  build first.
- The script builds `tutor-web` for the selected Rust target, copies it to the
  Tauri sidecar filename expected by v2, then merges
  `src-tauri/tauri.release.conf.json` for the bundle step.

Acceptance:

- A developer can produce a desktop artifact with one documented command.
- Build output path is printed clearly.

### Phase 5: First Release QA

Status: in progress.

Tasks:

- [x] Add desktop release QA automation script.
- [x] Add manual desktop QA checklist.
- [x] Add changelog and version bump script.
- [ ] Run `scripts/build-desktop.ps1` on a local machine with enough build time.
- [ ] Run `scripts/qa-desktop.ps1` against the built artifact.
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

Automation:

- `scripts/qa-desktop.ps1` validates the release app binary, Tauri sidecar
  filename, bundle directory when present, and starts the release `tutor-web`
  binary with a temporary data directory to smoke-test `/api/sessions` and
  `/api/knowledge-bases`; it also creates a test knowledge base and verifies
  that `settings.json` can be written through `/api/settings`.
- `docs/qa/desktop-v0.1.md` tracks the manual UI checks that cannot be safely
  completed by a shell script.

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

## 12. Desktop App Feel Follow-up

The first desktop release may still use the existing React UI, but the product
should not continue to feel like a web page embedded inside a small browser.
After the basic Tauri packaging path is stable, plan a desktop polish slice that
makes the app behave like a local workspace.

The follow-up should cover these areas at a product level:

- replace browser-default interactions with app-owned interactions where they
  are visible to users, including context menu entry points;
- keep the top-level app shell fixed and make scrolling local to panes such as
  chat history, Notebook tree, editors, trace panels, and inspectors;
- use desktop-native capabilities where appropriate, including file/folder
  pickers, opening external links in the system browser, revealing local files,
  and future app-level shortcuts or command palette behavior;
- define product-specific context menu capability areas for Notebook, Chat,
  Knowledge, Research, Books, and other work surfaces in later detailed design;
- preserve the existing local-first architecture and avoid rewriting backend
  routes as Tauri commands unless a native capability genuinely requires it.

Detailed right-click menu items and per-surface actions are intentionally out of
scope for this note and should be specified during the desktop polish design
slice.

### Phase 6: Desktop App Feel Polish

Status: in progress.

Tasks:

- [ ] Audit browser-default behaviors that are visible in the desktop app,
      including top-level scrolling, browser context menus, drag/drop defaults,
      in-window link navigation, focus outlines, and text/image selection where
      they conflict with app behavior.
- [x] Lock the top-level app shell so the window itself does not behave like a
      scrolling web page.
- [ ] Make scrolling pane-local for Chat history, Notebook tree, editors,
      Knowledge lists, Research reports, Trace, Settings, and other work areas.
- [x] Add an app-owned context menu framework that can replace the browser
      context menu in desktop mode.
- [x] Define high-level context menu capability areas for Notebook, Chat,
      Knowledge, Research, Books, Settings, and source references without
      finalizing individual menu items yet.
- [x] Implement the first context menu slice with generic Copy/Open Link/Copy
      Link actions, editable-field Cut/Copy/Paste/Select All actions, and
      Notebook file-tree Open/New/Copy Path/Delete actions, using native
      desktop clipboard commands instead of browser clipboard permission prompts.
- [x] Route external web links through the system browser in desktop mode.
- [ ] Use native desktop affordances for file/folder selection and revealing
      local files where product flows already need those actions.
- [ ] Add or plan app-level shortcuts and command palette behavior for common
      desktop workflows.
- [ ] Add desktop QA checks for browser-default interaction regressions, pane
      scrolling behavior, context menu ownership, and external link handling.

Acceptance:

- The app no longer exposes obvious browser context menus in normal desktop
  workflows.
- The top-level window stays fixed while the active work pane owns scrolling.
- External links and local file/folder actions use desktop-appropriate behavior.
- Each major surface has a documented context menu capability area, even if not
  every concrete menu item is implemented in the first polish slice.

## 13. Done Criteria for v0.1 Desktop Release

- A Windows desktop artifact exists.
- User can launch the app without terminal commands.
- Backend sidecar starts automatically.
- Frontend talks to the sidecar over local REST/WebSocket.
- LLM settings can be configured in the app.
- Chat works with streaming output.
- Knowledge base upload and RAG search work.
- Local data persists across restart.
- Build instructions are documented.

## 14. Remaining Implementation Order

1. Run `scripts/build-desktop.ps1` on a local Windows machine with enough build
   time and record the artifact path.
2. Run `scripts/qa-desktop.ps1` against that artifact and fix any packaging or
   sidecar issues it reports.
3. Complete the manual checklist in `docs/qa/desktop-v0.1.md` on a clean
   Windows profile or clean app data directory.
4. Validate `.github/workflows/release-desktop.yml` with `workflow_dispatch`
   once repository secrets and private dependency access are configured.
5. Decide whether the first shared artifact is a portable build, NSIS installer,
   MSI, or a combination.
6. Execute Phase 6 for native-app feel, context menu capability areas,
   pane-local scrolling, and native file/link behavior.
