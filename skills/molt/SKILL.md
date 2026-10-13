---
name: molt
description: Use MOLT to inventory JavaScript, TypeScript, and Java repositories before planning an evidence-based, agent-assisted rewrite into Rust. Trigger when asked to simplify a legacy full-stack repository and rebuild it in Rust.
---

# MOLT — Shed the legacy. Reborn in Rust.

MOLT is an agent workflow, not a one-shot converter. The CLI currently supports **file inventory only**; do not claim it detects symbols, dependencies, endpoints, or behavior. Read [workflow](references/workflow.md) before a migration. Keep the original repository as source of truth.

1. Run `molt analyze --source <legacy-repo>`; inspect the JSON result and confirm excluded directories and coverage. The source repository receives only a rebuildable `.molt/` SQLite index (ensure it is gitignored).
2. Run `molt query --source <legacy-repo> project-summary` and `molt query --source <legacy-repo> files --limit 25 --offset 0`. Inspect original files and tests directly for semantics; do not infer behavior from filename hashes.
3. Produce a written migration plan with evidence, functionality inventory, unknowns, target architecture, risks, milestones and verification gates. **Ask for explicit approval** before breaking changes, service merges, destructive schema changes, data deletion, or deployment.
4. Implement approved Rust modules in a **separate target directory**. Preserve observable behavior and document intentional differences. Agent writes code; MOLT does not invoke an LLM or generate application code.
5. Independently run build, clippy, unit/integration/contract/browser tests where applicable. Report unverified behavior as unverified; never claim deployment verified from chart generation alone.
6. Only after authorization, package, deploy to the selected k3s context and run smoke checks. Those CLI automation commands are **not implemented yet**; use existing project tools deliberately and report the exact evidence.

See [product scope and milestones](../../docs/product.md). Avoid putting large code dumps into context; query narrowly and use source locations when available.
