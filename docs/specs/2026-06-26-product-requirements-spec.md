# llm-tutor Product Requirements Spec

> Status: active | Date: 2026-06-26 | Scope: consolidate current and planned product requirements into one itemized spec.

## 1. Product Goal

- REQ-001: The product shall be a local-first AI learning workspace.
- REQ-002: The product shall support learning from user-provided documents, chat history, web sources, and generated reports.
- REQ-003: The product shall prioritize grounded learning workflows over general-purpose chat.
- REQ-004: The product shall keep agent runtime responsibilities in `llm-harness-runtime` / `llm-harness-agent` wherever possible.
- REQ-005: The product shall keep `llm-tutor` focused on product data, UI, knowledge bases, books, quizzes, reports, settings, and runtime-session mappings.

## 2. Architecture

- REQ-010: The system shall use a Rust workspace backend.
- REQ-011: The system shall expose a React web UI.
- REQ-012: The backend shall be split into `tutor-agent`, `tutor-tools`, `tutor-rag`, and `tutor-web`.
- REQ-013: `tutor-agent` shall own capability routing and agent prompts.
- REQ-014: `tutor-tools` shall own runtime tool implementations.
- REQ-015: `tutor-rag` shall own document indexing, embeddings, vector storage, and retrieval.
- REQ-016: `tutor-web` shall own HTTP APIs, WebSocket streaming, product stores, and session mapping.
- REQ-017: The web UI shall own product interaction, layout, settings, and rendering.
- REQ-018: The backend shall use runtime sessions for durable conversation history.
- REQ-019: The backend shall avoid building a parallel session/context/orchestration system when runtime APIs are available.

## 3. Session and Conversation

- REQ-030: Users shall be able to create a new chat session.
- REQ-031: Users shall be able to resume an existing chat session.
- REQ-032: Users shall be able to rename a session.
- REQ-033: Users shall be able to delete a session.
- REQ-034: Sessions shall persist user and assistant messages.
- REQ-035: Sessions shall persist selected capability.
- REQ-036: Sessions shall persist selected knowledge base.
- REQ-037: Sessions shall persist selected LLM config.
- REQ-038: Sessions shall persist selected search config.
- REQ-039: Sessions shall expose context usage information when available.
- REQ-040: Sessions shall support runtime compaction summaries.
- REQ-041: The UI shall show recent sessions in the sidebar.
- REQ-042: The chat view shall support streaming assistant output.
- REQ-043: The chat input shall remain usable while an agent is running when product behavior allows it.
- REQ-044: The system shall provide a way to stop or cancel a running agent turn. Status: planned.

## 4. Capability Modes

- REQ-050: The composer shall allow selecting a capability mode.
- REQ-051: The system shall support `chat` mode.
- REQ-052: The system shall support `deep_solve` mode.
- REQ-053: The system shall support `code_exec` mode.
- REQ-054: The system shall support `quiz` mode.
- REQ-055: The system shall support `research` mode.
- REQ-056: Capability changes shall not destroy the current runtime session.
- REQ-057: Capability-specific prompts shall be owned by `tutor-agent`.
- REQ-058: Capability-specific product UI shall be owned by `web-ui`.

## 5. Chat Mode

- REQ-070: Chat mode shall answer open-ended user questions.
- REQ-071: Chat mode shall be able to use `rag_search`.
- REQ-072: Chat mode shall be able to use `web_search`.
- REQ-073: Chat mode shall be able to use `web_fetch`.
- REQ-074: Chat mode shall be able to use `code_exec`.
- REQ-075: Chat mode shall require web search for current, external, or fact-collection requests.
- REQ-076: Chat mode shall avoid inventing facts when web search or fetch fails.
- REQ-077: Chat mode shall display citations only when the agent actually used a citation-producing tool.
- REQ-078: Chat mode shall not add backend-invented RAG citations when `rag_search` was not actually called.
- REQ-079: Chat mode shall support user-selected `@` references to Space artifacts. Status: implemented.
- REQ-080: `@` references shall be stored as structured artifact references, not only as plain text mentions. Status: implemented.
- REQ-081: The first supported `@` reference targets shall include Notebook entries, Quiz sessions, and Quiz questions. Status: implemented.
- REQ-082: The chat composer shall display selected `@` references as removable chips. Status: implemented.
- REQ-083: The `@` reference picker shall search Space artifacts by title, type, and useful metadata. Status: implemented.
- REQ-084: Mentioned Space artifact IDs shall be persisted with the user message and restored with the session. Status: implemented.
- REQ-085: Agents shall access mentioned Space artifacts through product tools such as `read_space_item` where possible, instead of blindly injecting long artifact content into every prompt. Status: implemented for Chat and Research.
- REQ-086: When an agent uses mentioned Space artifact content in an answer, the response shall identify the relevant artifact or cite it through the product reference system. Status: planned.

## 6. Attachments

- REQ-090: The chat composer shall allow selecting file attachments.
- REQ-091: The chat composer shall display selected attachments as removable chips.
- REQ-092: Text attachments shall be included as current-turn context.
- REQ-093: PDF attachments shall be parsed server-side and included as current-turn context.
- REQ-094: Attachment parsing failures shall show the failure reason.
- REQ-095: Attachment text shall be truncated to protect context capacity.
- REQ-096: Unsupported attachment types shall fail clearly instead of being silently ignored.
- REQ-097: Attachment content sent to the model shall not visually flood the user message bubble.

## 7. Knowledge Base and RAG

- REQ-110: Users shall be able to create a knowledge base.
- REQ-111: Users shall be able to name a knowledge base.
- REQ-112: Users shall choose embedding configuration when creating a knowledge base.
- REQ-113: Users shall be able to upload documents into a knowledge base.
- REQ-114: The system shall parse UTF-8 text files.
- REQ-115: The system shall parse Markdown files.
- REQ-116: The system shall parse PDF text.
- REQ-117: The system shall show upload and ingestion progress.
- REQ-118: Ingestion progress shall eventually include parse, chunk, embed, and vector-write stages.
- REQ-119: The system shall chunk documents before indexing.
- REQ-120: Chunk IDs shall be stable enough to support citation and document management.
- REQ-121: The system shall store vectors in LanceDB.
- REQ-122: The system shall store document metadata.
- REQ-123: The system shall support searching a knowledge base.
- REQ-124: `rag_search` shall return source chunks.
- REQ-125: Answers using RAG shall show citation sources.
- REQ-126: Users shall be able to view document chunks.
- REQ-127: Users shall be able to delete documents.
- REQ-128: Users shall be able to reindex documents.
- REQ-129: Users shall be able to preview uploaded documents.
- REQ-130: PDF preview shall render correctly. Status: needs hardening.
- REQ-131: Chunking shall be upgraded from basic splitting to paragraph/token-aware splitting. Status: planned.
- REQ-132: Retrieval quality shall support reranking or improved scoring. Status: planned.
- REQ-133: Citations shall support better location metadata such as page, paragraph, or chunk highlight. Status: planned.
- REQ-134: Web pages and fetched sources shall be ingestible into a knowledge base. Status: implemented for Research report sources through the existing Knowledge Base document ingestion path.

## 8. Embedding Configuration

- REQ-150: Users shall be able to add embedding provider configurations.
- REQ-151: Embedding configs shall include endpoint URL.
- REQ-152: Embedding configs shall include API key.
- REQ-153: Embedding configs shall include model ID.
- REQ-154: Embedding configs shall include vector dimensions.
- REQ-155: Embedding configs shall support sending or omitting the `dimensions` parameter.
- REQ-156: Knowledge bases shall store the embedding config they were created with.
- REQ-157: The UI shall warn when a knowledge base embedding config is missing or invalid. Status: planned.
- REQ-158: The system shall provide embedding provider health checks. Status: planned.

## 9. LLM Configuration

- REQ-170: Users shall be able to add LLM configurations.
- REQ-171: LLM configs shall support OpenAI-compatible APIs.
- REQ-172: LLM configs shall support Anthropic Messages APIs.
- REQ-173: LLM configs shall include base URL.
- REQ-174: LLM configs shall include API key.
- REQ-175: LLM configs shall include model ID.
- REQ-176: LLM configs shall include optional chat path.
- REQ-177: LLM configs shall include context window tokens.
- REQ-178: The composer shall allow selecting among configured LLM models.
- REQ-179: LLM model selection shall not require binding the model permanently to a session unless product behavior explicitly chooses to.
- REQ-180: The UI shall provide LLM provider health checks. Status: planned.
- REQ-181: The UI shall describe model capabilities such as tools, streaming, JSON schema, vision, and context window. Status: planned.
- REQ-182: API keys shall eventually be stored more securely than plain local config. Status: planned.

## 10. Web Search and Web Fetch

- REQ-200: The system shall provide a `web_search` tool.
- REQ-201: The system shall provide a `web_fetch` tool.
- REQ-202: Search provider configuration shall be editable in Settings.
- REQ-203: Search providers shall support URL and API key fields.
- REQ-204: Search providers shall support free and paid providers where possible.
- REQ-205: Search providers shall include Bing support.
- REQ-206: Search providers shall include Brave support.
- REQ-207: Search providers shall include Tavily support.
- REQ-208: Search providers shall include Serper support.
- REQ-209: Search providers shall include SerpAPI support.
- REQ-210: Search providers shall include Exa support.
- REQ-211: DuckDuckGo support shall avoid relying on low-quality Instant Answer API behavior.
- REQ-212: `web_fetch` shall extract readable page text.
- REQ-213: Search and fetch failures shall surface actionable reasons.
- REQ-214: Search results shall be deduplicated. Status: planned.
- REQ-215: Search results shall support source quality scoring. Status: planned.
- REQ-216: The system shall support academic search or scholar search. Status: planned.
- REQ-217: The system shall support news/current-events search. Status: planned.
- REQ-218: The system shall support site-restricted search. Status: planned.

## 11. Deep Solve

- REQ-230: Deep Solve shall render as a structured solving workflow.
- REQ-231: Deep Solve shall show planning stage.
- REQ-232: Deep Solve shall show solve steps.
- REQ-233: Deep Solve shall show verification evidence where available.
- REQ-234: Deep Solve shall keep the final answer as the main visible result.
- REQ-235: Deep Solve shall attach relevant tool evidence to stages.
- REQ-236: Deep Solve shall attach citations to the final answer.
- REQ-237: Deep Solve trace events shall persist across session reload.
- REQ-238: Users shall be able to ask follow-up questions about a specific step.
- REQ-239: Deep Solve shall handle missing plan output gracefully.
- REQ-240: Deep Solve shall allow stopping a running solve. Status: planned.

## 12. Code Execution

- REQ-250: The system shall provide a `code_exec` tool.
- REQ-251: Code execution shall support Python.
- REQ-252: Code execution shall support shell where allowed by runtime environment.
- REQ-253: Code execution shall be used for non-trivial numeric verification.
- REQ-254: Code execution errors shall be returned clearly.
- REQ-255: Code execution shall respect approval requirements when configured.
- REQ-256: Code execution shall rely on runtime/environment abstractions rather than product-specific execution loops.
- REQ-257: Stronger sandboxing shall be added before untrusted multi-user execution. Status: planned.

## 13. Quiz

- REQ-270: Users shall be able to generate a Quiz from a selected knowledge base.
- REQ-271: Users shall be able to generate a Quiz from conversation material.
- REQ-272: Users shall generate Quiz content from Chat using conversation context, attachments, selected knowledge sources, or user-referenced saved material; Notebook and Research report pages shall not provide independent Quiz generation entry points. Status: implemented for conversation, attachments, knowledge bases, and `@` Space references.
- REQ-273: Quiz generation shall retrieve source chunks when a knowledge base is selected.
- REQ-274: Quiz generation shall produce strict structured JSON.
- REQ-275: Quiz questions shall include answer options.
- REQ-276: Quiz questions shall include the correct answer.
- REQ-277: Quiz questions shall include explanation.
- REQ-278: Quiz questions shall include citations where source material exists.
- REQ-279: Users shall be able to answer Quiz questions.
- REQ-280: Users shall get immediate correctness feedback.
- REQ-281: Users shall see final score summary.
- REQ-282: Users shall see missed-question review.
- REQ-283: Quiz answer and explanation consistency shall be validated. Status: partially implemented with deterministic schema/citation checks; needs LLM verifier hardening.
- REQ-284: Quiz shall support multi-select questions. Status: planned.
- REQ-285: Quiz shall support short-answer judging. Status: planned.
- REQ-286: Quiz shall support wrong-answer review records. Status: planned.
- REQ-287: Quiz shall support adaptive difficulty. Status: planned.
- REQ-288: Quiz shall be exposed to Chat as an enabled agent capability/tool, not as an automatic "send equals generate" mode. Status: implemented with `propose_quiz_plan` and `create_quiz` product tools.
- REQ-289: When Quiz capability is enabled, normal chat turns shall allow discussion of scope, source material, difficulty, question type, learner level, citation strictness, and review goals before any Quiz is generated. Status: implemented through normal Chat plus optional `propose_quiz_plan`.
- REQ-290: The agent shall call a dedicated `create_quiz` product tool only after the user explicitly asks to generate, confirms a plan, or provides an unambiguous generation request. Status: implemented in tool design/prompt contract; needs behavioral QA across providers.
- REQ-291: Quiz generation shall separate user instruction from source material. The latest user message may guide topic and constraints, but it shall not be treated as factual source material unless no other source is available or the user explicitly says it is source material. Status: partially implemented through `kb_id`, `notebook_entry_id`, `source_text`, and `source_label`; needs stronger UI/agent guardrails.
- REQ-292: A successful `create_quiz` tool call shall save the Quiz into Quiz Bank and render an interactive Quiz card in Chat. Status: implemented.
- REQ-293: Quiz planning may use learner memory for personalization, but generated factual answers and citations shall remain grounded in selected source material. Status: implemented for first slice; L3 memory is passed only for personalization when review/practice intent is detected.
- REQ-294: Quiz generation shall include a verifier stage inside the controlled product flow, not as a separate free-form chat agent. Status: implemented with deterministic validation plus a structured LLM verifier running inside the runtime `WorkflowEngine`.
- REQ-295: The verifier stage shall receive only the source chunks, candidate question JSON, answer, explanation, citations, and supporting quote needed for review. Status: implemented.
- REQ-296: The verifier stage shall return structured review output such as `verdict`, `issues`, `supported_answer`, `explanation_consistent`, `citation_supports_answer`, and optional repair guidance. Status: partially implemented with `verdict` and `issues`; richer booleans and repair guidance are planned.
- REQ-297: The verifier shall judge only against supplied source material and shall not introduce external knowledge or new factual claims. Status: implemented in prompt contract; needs provider-behavior QA.
- REQ-298: Questions that fail deterministic validation or verifier review shall not be saved as final Quiz questions unless they are repaired and re-verified. Status: implemented for first slice through runtime workflow repair and final publish validation.
- REQ-299: The first verifier implementation may retry or repair failed questions once, then discard unresolved questions rather than publishing weakly grounded content. Status: implemented for first slice with a bounded runtime workflow repair loop.

Current Quiz verification flow:

1. `create_quiz` builds source material from one of these sources: selected knowledge base chunks, a Notebook entry, explicit `source_text`, or selected Chat/Space material.
2. If a usable LLM config exists, the quiz runtime workflow definition is validated before generation: `collect_sources -> generate_questions -> verify_questions -> publish_questions`, with a bounded repair edge from verifier back to generation.
3. `tutor_agent::quiz::generate_quiz_questions_with_workflow` runs the controlled quiz flow through runtime `WorkflowEngine`.
4. The `generate_questions` runtime LLM step submits structured single-choice question JSON through `submit_step_result`.
5. The `verify_questions` runtime LLM step reviews generated questions against the supplied source chunks and submits a structured pass/fail result.
6. Failed verifier results route back to `generate_questions` once for repair; unresolved failures do not reach final publish.
7. The `publish_questions` executor deterministically validates JSON shape, option consistency, citations, and supporting quotes before storage.
8. If no usable LLM config exists, the backend uses a deterministic fallback question generator from source chunks.
9. The backend converts verified questions into stored `QuizQuestion` records and maps cited source indices to `QuizCitation` metadata.
10. `validate_quiz_questions_for_storage` rejects empty question sets, empty stems, too-few options, missing correct option IDs, empty explanations, missing citations, and empty citation text.
11. The stored quiz receives a `QuizVerificationReport` with method `llm_verifier_and_citation_check` or `deterministic_fallback_citation_check`.

Remaining hardening: richer verifier booleans and repair guidance can be added later, but the runtime `WorkflowEngine` execution and bounded repair/retry loop are now implemented.

## 14. Research

- REQ-300: Users shall be able to select Research mode in chat.
- REQ-301: Research mode shall create a brief research plan.
- REQ-302: Research mode shall call `web_search` for external facts.
- REQ-303: Research mode shall call `web_fetch` for important sources.
- REQ-304: Research mode shall optionally call `rag_search` when a knowledge base is selected.
- REQ-305: Research mode shall produce a Markdown report.
- REQ-306: Research reports shall include a title.
- REQ-307: Research reports shall include a summary.
- REQ-308: Research reports shall include key findings.
- REQ-309: Research reports shall include analysis.
- REQ-310: Research reports shall include limitations.
- REQ-311: Research reports shall include follow-up questions.
- REQ-312: Research reports shall include sources.
- REQ-313: Research progress shall appear as compact status, not normal assistant bubbles.
- REQ-314: Research trace events shall include planning, search, read, and report completion.
- REQ-315: Research mode shall clearly state when search or fetch failed.
- REQ-316: Research mode shall not cite sources that were not actually searched or fetched.
- REQ-317: Research reports shall be saved as `NotebookEntry` records with `type = research_report`. Status: implemented.
- REQ-318: Research reports shall restore with structured metadata from Notebook entry data. Status: partially implemented; session reload restores report-like assistant messages and attached source metadata, while broader report/version restore hardening remains planned.
- REQ-319: The UI shall provide a dedicated ResearchReport component. Status: implemented.
- REQ-320: The UI shall show a dedicated source list under each Research report. Status: implemented.
- REQ-321: Research reports shall support regeneration/versioning. Status: implemented for the first slice with a report Regenerate action and Notebook report metadata carrying version and generation details.
- REQ-322: Research shall support longer-running multi-step/parallel deep research. Status: implemented for the first slice by allowing the runtime Research workflow search step to spawn independent subtopic agents through `sync_spawn_agent`.
- REQ-323: Research mode shall preserve normal conversational interaction for clarifying research goals, scope, source preferences, output format, depth, time range, and optional Notebook or Knowledge Base context before starting detailed research. Status: implemented for the first slice with prompt policy, plain-text fallback, streaming, plan proposal, UI confirmation entry, and behavioral coverage for clarification-before-workflow.
- REQ-324: Research mode shall provide a detailed research workflow that can be explicitly started after the user's need is clear. The workflow shall cover search, source reading, source selection, synthesis, citation checking, and report generation; it shall not be forced for every Research message. Status: implemented for the first runtime workflow slice; remaining hardening covers stop/cancel behavior and richer long-running research controls.

## 15. Books and Learning Records

- REQ-340: Users shall be able to view Books. Status: implemented.
- REQ-341: Users shall be able to create a Book. Status: implemented.
- REQ-342: Users shall be able to turn Notebook entries into Book chapters. Status: implemented.
- REQ-343: Users shall be able to browse Book chapters. Status: implemented.
- REQ-344: Book chapters shall store Markdown content. Status: implemented.
- REQ-345: Book chapters shall store source session ID when available. Status: implemented.
- REQ-346: Book chapters shall store source Notebook entry ID. Status: implemented.
- REQ-347: Users shall be able to rename Books. Status: planned.
- REQ-348: Users shall be able to delete Books. Status: planned.
- REQ-349: Users shall be able to rename chapters. Status: planned.
- REQ-350: Users shall be able to delete chapters. Status: planned.
- REQ-351: Users shall be able to reorder chapters. Status: planned.
- REQ-352: Users shall be able to edit chapter Markdown. Status: planned.
- REQ-353: Users shall be able to export Books or chapters as Markdown. Status: planned.
- REQ-354: Users shall be able to export Books or chapters as PDF. Status: planned.
- REQ-355: Users shall be able to save chat answers into Notebook first, then optionally into Books. Status: planned.
- REQ-356: Users shall be able to save Quiz summaries into Notebook first, then optionally into Books. Status: planned.
- REQ-357: Book content shall eventually be usable as context or RAG source. Status: planned.

## 16. Space Workspace

- REQ-600: Space shall be the project-level learning workspace.
- REQ-601: Space shall not be equivalent to Notebook.
- REQ-602: Space shall initially contain Notebook, Quiz Bank, and Student Profile modules.
- REQ-603: The product shall support one default Space before requiring multi-space management.
- REQ-604: Space shall eventually support multiple named learning/research spaces. Status: planned.
- REQ-605: Space shall organize durable learning artifacts and learning state.
- REQ-606: Chat shall remain the primary generation surface.
- REQ-607: Space shall remain the primary review and organization surface.
- REQ-608: Knowledge Base shall remain the primary original-source and retrieval surface.

## 17. Notebook

- REQ-620: Notebook shall be a module inside Space.
- REQ-621: Notebook shall store flexible learning records.
- REQ-622: Notebook entries shall support ordinary notes.
- REQ-623: Notebook entries shall support research reports.
- REQ-624: Notebook entries shall support chat answer excerpts.
- REQ-625: Notebook entries shall support source snippets.
- REQ-626: Notebook entries shall support quiz summaries.
- REQ-627: Notebook entries shall support Deep Solve results.
- REQ-628: Research reports shall be represented as Notebook entries with `type = research_report`.
- REQ-629: Notebook entries shall store Markdown content.
- REQ-630: Notebook entries shall store metadata for source session, source message, query, sources, or generated-by fields where available.
- REQ-631: Users shall be able to list Notebook entries. Status: planned.
- REQ-632: Users shall be able to open Notebook entries. Status: planned.
- REQ-633: Users shall be able to create Notebook entries manually. Status: planned.
- REQ-634: Users shall be able to edit Notebook entries. Status: planned.
- REQ-635: Users shall be able to delete Notebook entries. Status: planned.
- REQ-636: Research mode shall default to saving reports into Notebook rather than directly into Books. Status: planned.
- REQ-637: Users shall be able to send a Notebook entry to Books as a chapter. Status: implemented.
- REQ-638: Notebook entries shall remain a Markdown/plain-text workspace and shall not be indexed into RAG or vector stores. Status: planned.
- REQ-639: Users shall be able to `@` a Notebook entry in Chat and ask the agent to revise, expand, summarize, or reorganize it. Status: implemented.
- REQ-640: Agent-produced Notebook edits shall be previewed as a proposed Markdown replacement or diff before they are applied. Status: implemented as complete Markdown replacement preview.
- REQ-641: Applying an agent-produced Notebook edit shall require explicit user confirmation. Status: implemented.
- REQ-642: Applied agent-produced Notebook edits shall update entry metadata and create a Notebook memory event. Status: implemented.
- REQ-643: Direct Notebook write tools shall be gated by explicit user instruction and product approval flow, not by implicit model choice alone. Status: implemented by exposing only a preview tool to the agent; writes happen through user-confirmed product UI.
- REQ-780: Notebook shall evolve toward a Markdown-first, Obsidian-like knowledge workspace with connected notes. Status: planned.
- REQ-781: Notebook shall support wiki-style note links such as `[[Note Title]]` and `[[note-id|alias]]`. Status: planned.
- REQ-782: Notebook shall parse and display backlinks for notes that reference the current note. Status: planned.
- REQ-783: Notebook shall parse and filter tags from note Markdown and note metadata. Status: planned.
- REQ-784: Notebook shall support importing Markdown notes from files, folders, or zip bundles. Status: planned.
- REQ-785: Notebook import shall preserve Markdown content, frontmatter, unknown metadata, and wiki links where possible. Status: planned.
- REQ-786: Notebook shall support exporting one note, selected notes, or the full Notebook as Markdown files or a zip bundle. Status: planned.
- REQ-787: Notebook export shall include stable frontmatter for title, id, type, tags, source metadata, created time, and updated time. Status: planned.
- REQ-788: Notebook import/export shall be local and portable; exported notes shall remain readable outside this app. Status: planned.
- REQ-789: Notebook shall support agent-assisted organization proposals such as suggested links, tags, and duplicate-note merges, with explicit user confirmation before writes. Status: planned.
- REQ-790: Agent-assisted Notebook organization shall be primarily triggered from Chat, not from independent generation buttons inside Notebook. Status: planned.
- REQ-791: When users ask Chat to organize a mentioned Notebook entry, the agent shall read the exact entry first and then produce a reviewable proposal. Status: planned.
- REQ-792: Notebook organization proposals shall include enough structure to distinguish complete Markdown edits, tag suggestions, wiki-link suggestions, and duplicate-note merge suggestions. Status: planned.
- REQ-793: Users shall be able to apply or reject Notebook organization proposals from Chat. Status: planned.
- REQ-794: If users ask about their notes without explicitly using `@`, the agent may search Notebook for candidate entries before answering. Status: planned.
- REQ-795: Notebook search for unmentioned questions shall return candidate entry ids, titles, types, tags, snippets, and confidence or ranking metadata. Status: planned.
- REQ-796: If Notebook search finds high-confidence candidates, the agent shall read the relevant entries before answering and cite the used Notebook entries. Status: planned.
- REQ-797: If Notebook search is ambiguous, the agent shall ask the user to choose candidate notes or present the candidate list before making strong claims. Status: planned.
- REQ-798: If users ask to edit, tag, link, or merge Notebook content without `@`, the agent shall search first and ask for target confirmation before creating a write proposal. Status: planned.
- REQ-799: If Notebook search finds no relevant entries, the agent shall say no relevant Notebook content was found before falling back to general knowledge or other tools. Status: planned.
- REQ-800: Chat composer shall expose Notebook association through the same source selector as Knowledge Base association. Status: planned.
- REQ-801: The shared source selector shall support at least No source, Notebook, and one concrete Knowledge Base. Status: planned.
- REQ-802: Notebook association shall use only plain-text search/read over Notebook Markdown; it shall not use embeddings, LanceDB, or vector indexing. Status: planned.
- REQ-803: Notebook association shall be modeled separately from Knowledge Base ids; `kb` shall only identify real Knowledge Bases. Status: planned.
- REQ-804: The UI shall prevent selecting Notebook and Knowledge Base simultaneously from the shared source selector until a multi-source design exists. Status: planned.
- REQ-805: Notebook search results used by the agent shall include navigable Notebook source ids. Status: planned.
- REQ-806: Chat shall add an Organize mode for Notebook and Space organization workflows. Status: planned.
- REQ-807: Organize mode shall enable Notebook search/read and proposal-first organization tools by default. Status: planned.
- REQ-808: Code execution shall be treated as a tool available to suitable modes rather than a standalone user-facing Chat mode. Status: planned.
- REQ-809: Notebook import shall show skipped-file details after the final import, not only during preview. Each skipped item shall include the original file name or zip path and a human-readable reason. Status: implemented.
- REQ-810: Notebook zip or folder import shall create entries in a batch and persist the Notebook store once per import operation rather than rewriting the full store after each entry. Status: implemented.
- REQ-811: The desktop app shall support importing an Obsidian Vault by selecting a local folder through a native directory picker and recursively reading `.md` and `.markdown` files. Status: implemented.
- REQ-812: Notebook import shall explicitly report Obsidian attachments and embedded assets that are not imported yet, including images and other non-Markdown files referenced by Markdown. Status: implemented.
- REQ-813: Notebook persistence shall use a file-backed vault directory where note bodies are stored as individual Markdown files. Status: implemented.
- REQ-814: Notebook shall keep a lightweight index file for stable ids, relative paths, entry types, source mappings, timestamps, and product metadata that should not be stored only in Markdown body text. Status: implemented.
- REQ-815: Notebook import from Markdown or Obsidian Vault folders shall prefer the source file name stem as the entry title, while preserving `frontmatter.title` as metadata, subtitle, or alias. Status: implemented.
- REQ-816: Notebook file-backed storage shall preserve imported relative folder paths where possible. Status: implemented.
- REQ-817: Notebook create, edit, rename, delete, import, export, and agent-applied edit flows shall update Markdown files and the Notebook index consistently. Status: partially implemented; create, edit, delete, import, export, and agent-applied edits are covered, while explicit path rename/link-update workflows remain planned.
- REQ-818: Agent autonomous Notebook/Vault exploration shall be available only when the chat source selector is associated with Notebook. Status: planned.
- REQ-819: When Notebook is not associated and the user has not explicitly `@` mentioned a Notebook entry, the agent shall not silently search or read Notebook content. Status: planned.
- REQ-820: Explicit `@` Notebook references shall still allow reading the referenced entry through `read_space_item`, even when Notebook is not the associated source. Status: planned.
- REQ-821: Notebook/Vault maintenance capabilities such as edit proposals, link suggestions, tag cleanup, move/rename proposals, duplicate review, merge proposals, and new-note proposals shall be enabled only in Organize mode. Status: planned.
- REQ-822: Non-Organize modes may answer from explicitly referenced or associated Notebook content, but shall not initiate Notebook maintenance proposals unless the session is switched to Organize mode. Status: planned.
- REQ-823: Notebook exploration tools shall be vault-aware and return stable ids together with paths, titles, entry types, tags, snippets, and link/backlink metadata where available. Status: planned.
- REQ-824: Notebook maintenance tools shall remain proposal-only; applying changes shall require user confirmation and shall execute through product Notebook APIs, not direct agent file-system writes. Status: planned.
- REQ-825: Notebook file browsing shall use lightweight tree/list metadata and shall not return full Markdown bodies in the file browser payload. Status: implemented.
- REQ-826: Notebook shall load full Markdown content only when a note is opened or explicitly read by an agent/product tool. Status: implemented.
- REQ-827: Notebook shall avoid a blocking full Vault rescan every time the Notebook tab is opened or revisited. Status: implemented.
- REQ-828: Notebook shall provide an explicit Vault refresh/reconcile operation for users to force external file changes to be indexed. Status: implemented.
- REQ-829: Notebook indexing shall track file stats such as relative path, modified time, and size so unchanged files can be skipped during refresh. Status: implemented.
- REQ-830: The desktop app shall use a file watcher for bound Notebook Vault directories where possible, with debounced index updates. Status: implemented.
- REQ-831: Notebook shall keep folder expansion state and selected note state stable across tab switches and non-destructive refreshes. Status: implemented.
- REQ-832: Notebook shall show indexing status such as watching, refreshing, last refreshed time, and changed-file count when available. Status: implemented.
- REQ-833: Notebook file tree rendering shall be designed to support large Vaults, including lazy folder expansion or virtualization when needed. Status: implemented with virtualized visible-row rendering.
- REQ-834: Notebook shall use an editor-style data model where the Vault files, Notebook index, file watcher, frontend explorer, open note buffer, and relation panels are separate responsibilities. Status: planned.
- REQ-835: Opening the Notebook page shall render from the current lightweight index first; full Markdown bodies and selected-note relations shall load only after a note is opened. Status: implemented for note body loading; relation-panel loading remains planned.
- REQ-836: Notebook create-note and create-folder actions shall be scoped to the currently selected folder or an explicit target folder. Status: planned.
- REQ-837: Notebook rename, move, and delete actions shall update the Markdown file path and Notebook index together and shall report affected links when link rewriting is not performed automatically. Status: planned.
- REQ-838: Notebook shall render Obsidian-style `[[wiki links]]` as navigable internal links and shall provide a clear unresolved-link path such as creating the missing target note. Status: partially implemented; link parsing and basic navigation exist, unresolved-link creation remains planned.
- REQ-839: Notebook shall provide a collapsible selected-note information panel for backlinks, outgoing links, tags, source metadata, and local graph. Status: partially implemented; panel and collapse behavior exist, richer relation detail remains planned.
- REQ-840: Notebook shall preserve user Markdown as much as possible, including unknown frontmatter, comments, aliases, and Obsidian-compatible syntax, unless the user explicitly applies a normalization or organization proposal. Status: planned.
- REQ-841: Saving generated content to Notebook shall let the user choose the target vault folder or create a new folder before the entry is written. Status: implemented for Research report and Research chat answer save flows.

## 18. Quiz Bank

- REQ-650: Quiz Bank shall be a module inside Space.
- REQ-651: Quiz Bank shall show historical quizzes.
- REQ-652: Quiz Bank shall show quiz scores.
- REQ-653: Quiz Bank shall show missed questions.
- REQ-654: Quiz Bank shall show explanations and citations.
- REQ-655: Quiz Bank shall support review and re-practice. Status: planned.
- REQ-656: Quiz Bank shall not be the primary quiz generation surface.
- REQ-657: Quiz generation shall remain in chat through the composer Quiz mode.
- REQ-658: The standalone Quiz navigation page shall be removed after Quiz Bank exists in Space.
- REQ-659: Quiz history shall move from the current Quiz page into Space / Quiz Bank.
- REQ-660: Quiz Bank shall support filtering by source type later. Status: planned.
- REQ-661: Users shall be able to `@` a Quiz session or Quiz question in Chat for explanation, follow-up practice, review, or related quiz generation. Status: implemented.
- REQ-662: Quiz-related `@` references shall preserve enough metadata for the agent to read the original prompt, question text, answer, explanation, citations, and user answer where available. Status: implemented.

## 19. Student Profile

- REQ-670: Student Profile shall be a module inside Space.
- REQ-671: Student Profile shall summarize what the student has studied.
- REQ-672: Student Profile shall summarize strengths.
- REQ-673: Student Profile shall summarize weaknesses.
- REQ-674: Student Profile shall summarize recent activity.
- REQ-675: Student Profile shall summarize quiz stats.
- REQ-676: Student Profile shall summarize recent notebook topics.
- REQ-677: Student Profile shall recommend next actions.
- REQ-678: Student Profile shall start with deterministic stats before complex LLM memory. Status: planned.
- REQ-679: Student Profile shall remain explainable. Status: planned.
- REQ-680: Student Profile shall eventually be editable or user-correctable. Status: planned.
- REQ-681: Student Profile shall use Markdown memory documents as the initial durable source of truth. Status: planned.
- REQ-682: Student Profile may later use a structured cache or projection derived from Markdown memory. Status: planned.
- REQ-683: Student Profile shall not require a separate hidden student-profile database in the MVP. Status: planned.

## 20. Memory System

- REQ-690: The product shall provide a Memory module. Status: planned.
- REQ-691: Memory shall use readable Markdown files as the primary durable representation in the MVP. Status: planned.
- REQ-692: Memory shall support an L1 raw-event layer. Status: planned.
- REQ-693: L1 shall include chat events. Status: planned.
- REQ-694: L1 shall include quiz events. Status: planned.
- REQ-695: L1 shall include notebook events. Status: planned.
- REQ-696: L1 shall include research events. Status: planned.
- REQ-697: Memory shall support an L2 per-surface summary layer. Status: planned.
- REQ-698: L2 shall include `chat.md`. Status: planned.
- REQ-699: L2 shall include `quiz.md`. Status: planned.
- REQ-700: L2 shall include `notebook.md`. Status: planned.
- REQ-701: L2 shall include `research.md`. Status: planned.
- REQ-702: Memory shall support an L3 cross-surface memory layer. Status: planned.
- REQ-703: L3 shall include `recent.md`. Status: planned.
- REQ-704: L3 shall include `profile.md`. Status: planned.
- REQ-705: L3 shall include `scope.md`. Status: planned.
- REQ-706: L3 shall include `preferences.md`. Status: planned.
- REQ-707: L3 shall include `teaching_strategy.md`. Status: planned.
- REQ-708: Memory entries shall support stable hidden entry ids. Status: planned.
- REQ-709: Memory entries shall support source references. Status: planned.
- REQ-710: Memory entries shall be editable by the user. Status: planned.
- REQ-711: Memory consolidation shall initially be manually triggered from the Memory module. Status: planned.
- REQ-712: Memory consolidation shall show which source layers or surfaces will be used. Status: planned.
- REQ-713: Memory consolidation shall write back to Markdown files. Status: planned.
- REQ-714: The system may later suggest consolidation after N turns, quiz completion, or research-report saves. Status: planned.
- REQ-715: The product shall provide a `read_memory` tool for agents. Status: planned.
- REQ-716: Agents shall call `read_memory` when personalized teaching, quiz generation, review planning, or long-running learning context requires it. Status: planned.
- REQ-717: Memory shall not be injected wholesale into every prompt by default. Status: planned.
- REQ-718: `write_memory` shall be limited to explicit user preferences or user-approved facts. Status: planned.
- REQ-719: Memory content shall guide teaching behavior and personalization, not act as factual source material for external facts. Status: planned.
- REQ-720: Memory Markdown footnote refs shall render as clickable inline source chips. Status: planned.
- REQ-721: Clicking an inline memory source chip shall scroll to the corresponding bottom reference item. Status: planned.
- REQ-722: Clicking a bottom memory reference item shall navigate to the related Chat, Notebook, Quiz, Research, Book, or Knowledge Base surface when possible. Status: planned.
- REQ-723: Internal memory entry markers such as `<!--m_xxx-->` shall never be displayed in rendered Markdown. Status: planned.

## 21. Markdown Rendering

- REQ-370: Assistant messages shall render Markdown.
- REQ-371: Markdown rendering shall support GitHub-flavored Markdown tables.
- REQ-372: Markdown rendering shall support math.
- REQ-373: Markdown rendering shall support KaTeX.
- REQ-374: Markdown rendering shall support soft line breaks.
- REQ-375: Markdown rendering shall support heading anchors.
- REQ-376: Markdown rendering shall support external links safely.
- REQ-377: The system shall not attempt to fix invalid LLM Markdown that is semantically malformed.
- REQ-378: The system prompt should encourage valid Markdown for tables and math.
- REQ-379: Product source references shall use dedicated UI behavior rather than relying only on raw Markdown footnote rendering. Status: planned.

## 22. Trace, Status, and UX

- REQ-390: Tool calls shall emit trace events.
- REQ-391: Tool results shall emit trace events.
- REQ-392: Long-running workflows shall emit status events.
- REQ-393: Some status events shall update in place rather than produce many visible bubbles.
- REQ-394: Progress should feel like an assistant message when product-relevant.
- REQ-395: Debug trace shall be available in a collapsible right panel.
- REQ-396: The right trace panel shall default to collapsed.
- REQ-397: The user-facing chat area shall not be visually separated from trace by a heavy divider.
- REQ-398: Failures shall show the reason, not only a generic failed state.
- REQ-399: The UI shall distinguish final answer text from progress/thinking/status text.

## 23. Settings

- REQ-410: Settings shall use tab-like sections.
- REQ-411: Settings shall include appearance controls.
- REQ-412: Settings shall include status/diagnostics.
- REQ-413: Settings shall include network/search configuration.
- REQ-414: Settings shall include LLM configuration.
- REQ-415: Settings shall include embedding configuration.
- REQ-416: Settings shall include capability/tool configuration.
- REQ-417: Settings shall include memory/session configuration when available.
- REQ-418: Settings shall support adding multiple LLM configs.
- REQ-419: Settings shall support adding multiple embedding configs.
- REQ-420: Settings shall support adding multiple search configs.
- REQ-421: Settings shall support save/apply behavior.
- REQ-422: Settings shall show unsaved changes clearly.
- REQ-423: Settings shall provide provider health checks. Status: planned.
- REQ-424: Settings shall support config import/export. Status: planned.

## 24. Navigation and Layout

- REQ-440: The app shall have a left sidebar.
- REQ-441: The sidebar shall include Chat.
- REQ-442: The sidebar shall include Tutor/assistant entry.
- REQ-443: The sidebar shall include Writing entry.
- REQ-444: The sidebar shall include Books.
- REQ-445: The sidebar shall include Knowledge Base.
- REQ-446: The sidebar shall include Quiz.
- REQ-447: The sidebar shall include Space.
- REQ-448: The sidebar shall include Memory.
- REQ-449: The sidebar shall include Settings.
- REQ-450: The sidebar shall show recent sessions.
- REQ-451: The sidebar shall be collapsible.
- REQ-452: Recent sessions shall support rename and delete.
- REQ-453: Clicking Chat shall open a new conversation by default.
- REQ-454: The new conversation view shall center the composer.
- REQ-455: The chat scroll behavior shall match common chat apps.
- REQ-456: The UI shall remain usable on common desktop sizes.
- REQ-457: Mobile responsiveness shall be improved. Status: planned.

## 25. Storage and Data

- REQ-470: Product data shall be stored locally for MVP.
- REQ-471: Knowledge base metadata shall be durable.
- REQ-472: Quiz sessions shall be durable.
- REQ-473: Books shall be durable.
- REQ-474: Runtime sessions shall be durable through runtime session storage.
- REQ-475: Trace events shall be restorable where relevant.
- REQ-476: Compact summaries shall be restorable where runtime supports them.
- REQ-477: The project shall evaluate SQLite when JSON stores become limiting.
- REQ-478: Storage paths shall be documented.
- REQ-479: Migration strategy shall be defined before changing persisted schemas. Status: planned.

## 26. Testing and Quality

- REQ-490: Frontend production build shall pass.
- REQ-491: `tutor-agent` mock integration tests shall pass.
- REQ-492: `tutor-web` library tests shall pass.
- REQ-493: RAG retrieval shall be testable without a real LLM.
- REQ-494: Quiz generation shall have non-real-LLM tests.
- REQ-495: Research trace events shall have mock tests.
- REQ-496: Book store and API shall have tests.
- REQ-497: Attachment parsing shall have tests.
- REQ-498: `tutor-web` startup smoke test shall be added. Status: planned.
- REQ-499: `cargo clippy --workspace --all-targets --all-features -- -D warnings` shall be run and cleaned up or documented. Status: planned.
- REQ-500: CI shall run Rust tests and frontend build. Status: planned.

## 27. Documentation

- REQ-510: README shall describe current capabilities accurately.
- REQ-511: README shall describe startup commands accurately.
- REQ-512: README shall describe current limitations accurately.
- REQ-513: Product roadmap shall match implemented state.
- REQ-514: Research plan shall track implemented and planned work.
- REQ-515: Framework gaps shall be recorded in `docs/framework-feedback.md`.
- REQ-516: Runtime/API issues shall be raised upstream when appropriate.
- REQ-517: Real-provider test requirements shall be documented. Status: planned.

## 28. Security and Privacy

- REQ-530: The app shall be considered single-user local-first in MVP.
- REQ-531: API keys shall not be logged in trace events.
- REQ-532: API keys shall be masked in the UI.
- REQ-533: Uploaded documents shall stay local unless explicitly sent to providers for embedding or LLM context.
- REQ-534: Code execution shall not be treated as safe for hostile multi-user input.
- REQ-535: Multi-user auth and permissions shall be out of MVP scope.
- REQ-536: Stronger sandboxing shall be required before hosted/multi-user deployments. Status: planned.

## 29. Desktop Release

- REQ-560: The product shall be packaged as a Tauri desktop application. Status: implemented; manual desktop QA pending.
- REQ-561: The desktop app shall bundle the React UI production build. Status: implemented; release artifact QA pending.
- REQ-562: The desktop app shall start the local Rust backend automatically. Status: implemented; manual desktop QA pending.
- REQ-563: The desktop app shall not require users to run `npm run dev` or `cargo run` manually. Status: implemented for release builds; manual desktop QA pending.
- REQ-564: The desktop app shall use an OS-appropriate application data directory by default. Status: implemented.
- REQ-565: The desktop app shall keep API keys user-configured and shall not bundle provider credentials. Status: implemented.
- REQ-566: The first desktop release shall prioritize Windows packaging. Status: implemented in build scripts; artifact QA pending.
- REQ-567: The release process shall support reproducible builds through scripts or CI. Status: implemented; CI secret validation pending.
- REQ-568: The app shall preserve the local-first storage model in the desktop release. Status: implemented.
- REQ-569: Desktop packaging details shall be tracked in `docs/plans/2026-06-28-tauri-desktop-release-plan.md`. Status: implemented.
- REQ-570: Every future public desktop release shall include a macOS artifact, preferably a `.dmg` built on macOS and uploaded to the same GitHub Release as the Windows installers. Status: planned; GitHub Actions path documented but not validated.
- REQ-571: The desktop app shall hide or replace browser-default interactions that make the product feel like an embedded webpage, including visible browser context-menu behavior. Status: in progress; app-owned context menus, native clipboard use, external-link routing, and file-drop interception are implemented.
- REQ-572: The desktop app shall use a fixed application shell where top-level window scrolling is avoided and scroll behavior is owned by specific panes or work areas. Status: in progress; top-level shell and Chat/Trace panes are hardened, full surface audit remains.
- REQ-573: The desktop app shall provide product-owned context menu capability areas for major surfaces such as Notebook, Chat, Knowledge, Research, and Books, with detailed menu items specified during implementation design. Status: in progress; framework and first Notebook/generic action slice are implemented.
- REQ-574: The desktop app shall prefer native desktop affordances for file/folder selection, revealing local files, external link opening, and future app-level shortcuts where appropriate. Status: in progress; shared directory picker helper, Notebook Vault folder binding, native clipboard, data directory reveal, and external-link opening are implemented.
- REQ-575: Desktop polish shall preserve the local-first sidecar architecture and shall not rewrite existing backend routes as Tauri commands unless a native capability requires it. Status: implemented.

## 30. Acceptance Baseline

- REQ-550: A clean clone shall be able to start backend and frontend using README commands.
- REQ-551: A user shall be able to create a knowledge base and upload a document.
- REQ-552: A user shall be able to ask a RAG-grounded question and see citations.
- REQ-553: A user shall be able to run a Deep Solve turn and inspect stages.
- REQ-554: A user shall be able to generate and answer a Quiz.
- REQ-555: A user shall be able to run a Research turn and get a sourced report.
- REQ-556: A user shall be able to save a Research report into a Book.
- REQ-557: A user shall be able to reopen the app and find prior sessions and Books.
