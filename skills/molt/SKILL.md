---
name: molt
description: Use MOLT to inventory JavaScript, TypeScript, and Java repositories before planning an evidence-based, agent-assisted rewrite into Rust. Trigger when asked to simplify a legacy full-stack repository and rebuild it in Rust.
---

# MOLT — Shed the legacy. Reborn in Rust.

MOLT is an agent workflow, not a one-shot converter. The CLI supports file inventory, **top-level JS/TS syntax, relative static import file edges, and limited GraphQL/Spring HTTP syntax contracts**. It does not resolve types, packages/aliases, full Java semantics, verified cross-service dependencies or behavior. Read [workflow](references/workflow.md) before a migration. Keep the original repository as source of truth.

1. Run `molt analyze --source <legacy-repo>`; inspect the JSON result and confirm excluded directories and coverage. The source repository receives only a rebuildable `.molt/` SQLite index (ensure it is gitignored).
2. Run `molt query --source <legacy-repo> project-summary` and `molt query --source <legacy-repo> files --limit 25 --offset 0`; optionally use `symbols <exact-name>`, `imports <relative-path>`, `dependents <relative-path>`, `contracts --category graphql|http|outbound_http_candidate`, `contract-issues --limit 25` or `http-links --route <literal-path>` for bounded syntax queries. Check `parse_status`, `contract_status`, `contract-issues` and `resolution`; `http-links` contains **unverified path/method candidates**, not proven calls. Do not treat unresolved/ambiguous/package/asset specifiers as resolved links. Inspect original files and tests directly for semantics; do not infer behavior from names or file-level edges.
3. Produce a written migration plan with evidence, functionality inventory, unknowns, target architecture, risks, milestones and verification gates. **Ask for explicit approval** before breaking changes, service merges, destructive schema changes, data deletion, or deployment.
4. Implement approved Rust modules in a **separate target directory**. Apply the project's `rust-best-practices` Skill when available: simplify the **source** architecture, while keeping the **target Rust** idiomatic, well-structured and thoroughly tested. Preserve observable behavior and document intentional differences. Agent writes code; MOLT does not invoke an LLM or generate application code.
5. Independently run build, clippy, unit/integration/contract/browser tests where applicable. Report unverified behavior as unverified; never claim deployment verified from chart generation alone.
6. Only after authorization, package, deploy to the selected k3s context and run smoke checks. Those CLI automation commands are **not implemented yet**; use existing project tools deliberately and report the exact evidence.

See [product scope and milestones](../../docs/product.md). Avoid putting large code dumps into context; query narrowly and use source locations when available.
