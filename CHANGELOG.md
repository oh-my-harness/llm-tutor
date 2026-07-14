# Changelog

All notable changes to this project will be documented in this file.

This project follows Semantic Versioning for source releases. Desktop bundle
versions must use numeric `MAJOR.MINOR.PATCH` values because Windows MSI does
not accept SemVer pre-release identifiers. Mark alpha/beta builds with Git tags,
release titles, or artifact names such as `v0.1.0-alpha.1`.

## Unreleased

## 0.2.1 - 2026-07-14

### Added

- Persistent cool-light and graphite-dark application themes.
- A redesigned Memory maintenance workspace with selectable model and action,
  background run indicators, progress recovery, evidence-backed diffs, and
  per-change review.
- Layered Memory evidence: L2 reads L1 product events, while ordinary L3 runs
  read stable L2 entries and retain source provenance.
- A root user manual covering current product workflows and the planned Tutor
  capability.

### Changed

- Redesigned chat presentation with full-width Agent answers, higher-contrast
  user messages, hover actions, and theme-matched scrollbars.
- Made complete session rows clickable while preserving selection, pinning,
  ordering, and background activity behavior.
- Memory generation now follows target-specific L2/L3 tool boundaries, rejects
  unread evidence citations, repairs oversized proposed changes, and honors the
  selected interface language for newly generated memory.
- Research no longer owns a separate Memory category. Research conversation is
  summarized through Chat, and saved reports enter Memory through Notebook.
- Removed the Books surface and the legacy Memory consolidation path.
- Desktop release assets now include platform and architecture in their names.
- Hardened desktop development startup against stale Vite and `tutor-web`
  processes and serialized backend build/launch.

### Fixed

- Restored Memory run state and per-module indicators after workspace
  navigation.
- Fixed repeated L2 workflow failures caused by declaring tools that were not
  mounted for the active memory layer.
- Fixed invalid and aliased Memory evidence references across Notebook, Quiz,
  Chat, and Knowledge events.
- Fixed Memory overview alignment, composer model icon sizing, and dropdown
  icon surfaces.

### Known Gaps

- API keys are stored in local JSON for now; system keychain integration is a
  later hardening task.
- Running workflows do not yet resume from their exact interruption point after
  a full application restart.
- Memory L2 freshness warnings, apply-time source revision checks, and the full
  Teaching Strategy dependency order remain pending.
- Linux packaging and automatic updates are not yet available.
