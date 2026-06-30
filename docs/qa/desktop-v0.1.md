# Desktop v0.1 QA Checklist

Use this checklist after running:

```powershell
.\scripts\build-desktop.ps1
.\scripts\qa-desktop.ps1
```

## Environment

- Date:
- Windows version:
- Build target:
- Artifact path:
- Clean profile or clean app data directory:

## Manual Checks

- [ ] Install or unpack the app on a clean Windows profile or after clearing the app data directory.
- [ ] Start the app without running `cargo run`, `cargo tauri dev`, or `npm run dev`.
- [ ] Confirm the app opens the React UI and does not show Vite proxy errors.
- [ ] Confirm Settings shows the desktop data directory and the Open button opens it.
- [ ] Change one setting, restart the app, and confirm it was restored from `settings.json`.
- [ ] Configure one LLM provider.
- [ ] Send a chat message and confirm streaming output.
- [ ] Configure one embedding provider.
- [ ] Create a knowledge base and upload a text file.
- [ ] Create a knowledge base and upload a PDF file.
- [ ] Ask a RAG question and confirm citation links appear only after `rag_search` was used.
- [ ] Generate a Quiz from conversation material.
- [ ] Generate a Quiz from knowledge base material.
- [ ] Save a Research report to Notebook.
- [ ] Close and restart the app.
- [ ] Confirm sessions, knowledge bases, notebooks, quizzes, and memory still exist after restart.
- [ ] Confirm closing the app stops the `tutor-web` sidecar process.
- [ ] Inspect visible logs and trace output for accidental API key exposure.

## Result

- [ ] Pass
- [ ] Fail

Notes:
