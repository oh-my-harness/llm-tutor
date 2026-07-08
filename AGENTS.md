# Development Principles

- Use `llm-harness-runtime` / `llm-harness-agent` first. Do not reimplement
  session, context building, tool orchestration, hooks, trace, compaction, or
  provider behavior in this repo.
- Keep `llm-tutor` focused on product data and UI: knowledge bases, documents,
  spaces, notebooks, quizzes, settings, and mappings to runtime session IDs.
- For durable conversation history, prefer runtime sessions such as
  `AgentHarness::with_session` and runtime session repos.
- If the framework API is awkward or missing a needed capability, record it in
  `docs/framework-feedback.md` instead of silently building a parallel system.
- When diagnosing problems, prefer a root-cause design fix over accumulating
  patches that make the project heavier or harder to reason about.
- Keep adapters between product code and runtime code thin, explicit, and
  covered by boundary tests.
- After completing a meaningful task, commit the changes promptly.
