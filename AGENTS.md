# MOLT agent instructions

MOLT is an agent-native Rust CLI plus an Agent Skill for evidence-based modernization of JavaScript/TypeScript/Java systems. **Simplification applies to the source system's unnecessary architecture and abstractions, not to the quality or rigor of the target Rust application.** Preserve observable behavior while building well-structured, idiomatic Rust. Do not describe syntax inventory, generated files, or Helm charts as verified conversion or deployment.

## Where to look

- Product scope and implementation status: `docs/product.md`.
- Migration workflow and human approval gates: `skills/molt/SKILL.md` and `skills/molt/references/workflow.md`.
- Rust implementation/review guidance: `.agents/skills/rust-best-practices/SKILL.md` (load its references as needed, not all at once).

## Repository boundaries

- `crates/molt-cli/` is the machine-readable CLI; `crates/molt-index/` holds local, rebuildable code intelligence. The original repository is the source of truth, not SQLite.
- `test_subjects/` contains ignored, independent legacy projects. Do not commit or copy their source, secrets, environment files, or `.molt/` database into this repository. Unless expressly asked to change a fixture, only read/analyze these projects.
- Keep migration output in a separate directory; never overwrite the input. Require explicit approval before breaking changes, service merges, destructive data/schema changes, deleting data, or deploying to Kubernetes.
- Do not add an embedded LLM, API key requirement, MCP server, or agent extension. Rust CLI operations must be deterministic and JSON-oriented; do not output unbounded source dumps or secrets.

## General-purpose tool boundary

- MOLT is a **general-purpose analysis and migration tool**, not an implementation of any `test_subjects/` application. The tool discovers business rules, names, API paths, data models and configurations from the analyzed repository; it must never assume or embed a particular subject's route, service name, directory layout, schema variable name, business parameter or workflow in production code or defaults.
- Use language/framework-level syntax and documented, reusable adapters (e.g. GraphQL SDL or Spring annotations), not observations about one fixture, to recognize candidate contracts. If an inference depends on a convention or cannot be confirmed statically, mark it as a candidate/unknown. Unsupported ecosystems are reported honestly, not generalized from a single sample.
- Tests should use independent synthetic examples to prove the same feature works across different names and directory layouts. Keep actual subject-specific observations and evidence in `docs/test-subjects.md` or separate migration plans, never as conditions in the CLI/indexer. Before adding any extraction rule, ask whether it would work unchanged for an unrelated repository.

## When modifying Rust

- Follow the Rust Skill above. Choose target Rust structure, type safety, explicit failure handling, tests and documentation based on actual requirements and established Rust patterns; do not cut these corners to make the target smaller. Work with the workspace's declared edition/toolchain; do not silently change either.
- Write tests for success, malformed input, stale-index deletion, and boundary conditions appropriate to the changed behavior. Keep file/line evidence and mark inferred or unsupported relationships as such.
- Before claiming a Rust change is complete, run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`. If a check cannot pass, report the exact failure; do not claim success or hide pre-existing errors.
- For proposed target applications, verify behavior against the legacy baseline. Cargo passing is necessary for Rust quality but insufficient to prove functional parity, safe database migration, or successful deployment.
