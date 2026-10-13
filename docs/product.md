# MOLT product baseline

Source: [product discussion](https://chatgpt.com/share/6ace211b-a838-83e8-9d77-f4a436e48b09), final naming decision: **MOLT**. *Shedding legacy. Reborn in Rust.* Agent-native Rust rewrite engine: less abstraction, less infrastructure, more Rust.

## Design commitments

- Interface: Rust CLI with bounded JSON output + portable Agent Skill for Pi, Codex and Claude Code; no built-in LLM, API key, MCP server, browser dashboard or agent extension.
- Inputs: JS/TS (including React/Node), Java/Spring, whole monorepos. Original source is read-only except for local, gitignored MOLT state. Rewrite output stays separate.
- Workflow: discover → reason/simplify → approve → rewrite → verify → package/deploy → report. Preserve observable behavior and distinguish evidence from inference.
- Intended output stack: Rust/Dioxus/WASM frontend, Axum/Tokio backend, SQLx/PostgreSQL, Redis as optional cache, PostgreSQL chunked binary media storage, OCI + Helm for single-node k3s/Traefik. Target architecture is a proposal requiring workload-specific validation, not a guarantee.
- Destructive migrations, data deletion, service merges, breaking changes and real deployment require explicit human approval. Charts alone do not prove deployability.
- SQLite stores rebuildable codebase intelligence (indexes, future semantic graph, findings, tasks, verification). Important approvals/plans belong in version-controlled artifacts. Queries must be bounded and traceable to source; incremental reindexing must account for affected dependents when semantic analysis is added.

## Implementation status / roadmap

| Stage | Scope | Status |
|---|---|---|
| 0 | Rust workspace, JSON CLI, JS/TS/Java file hashes + local SQLite, incremental upsert/removal, bounded file query, portable skill | Implemented (file inventory only) |
| 1 | Oxc JS/TS/JSX/TSX parser; symbol spans, imports, edges, evidence/confidence, affected-dependent invalidation; Java/OpenRewrite integration (requires Java tooling) | Planned |
| 2 | Project semantic IR, FTS5/search, contextual queries, findings, persistent tasks/decisions, resumable analysis versions | Planned |
| 3 | Rules/checks and scaffold, reproducible Rust verification with behavior/contract baselines and structured diagnostics | Planned |
| 4 | OCI packaging, Helm templates, opt-in deployment and smoke tests, performance/compatibility report | Planned |

Do not present the file inventory as AST analysis or call the CLI a working converter. Evaluate supply-chain, licensing, crates.io/GitHub name and trademark availability before public release; `molt` availability is not confirmed.
