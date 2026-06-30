# Changelog

All notable changes to this project will be documented in this file.

This project follows Semantic Versioning. Pre-release desktop builds should use
versions such as `0.1.0-alpha.1`, `0.1.0-alpha.2`, and `0.1.0-beta.1`.

## Unreleased

### Added

- Tauri desktop shell with managed `tutor-web` sidecar.
- Windows release build and QA scripts.
- Desktop app data directory support.
- Backend Settings Store at `<data-dir>/settings.json`.
- Chat, RAG, Knowledge Base, Quiz, Research, Notebook, Book, Space, and Memory
  prototype workflows.

### Changed

- Desktop release data is stored under the OS app data directory instead of the
  repository checkout.
- Frontend settings now prefer the backend Settings Store and keep
  localStorage only as a compatibility cache.

### Known Gaps

- Desktop v0.1 still needs a full manual QA pass before public release.
- API keys are stored in local JSON for now; system keychain integration is a
  later hardening task.
- macOS and Linux packaging are not yet automated.
