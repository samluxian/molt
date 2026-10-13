---
name: rust-best-practices
description: Write, review, test, or refactor idiomatic and maintainable Rust in MOLT or its approved migration outputs. Use for ownership, API and error design, async/database boundaries, unsafe code, and Rust verification; apply only rules relevant to the task.
license: MIT OR Apache-2.0
---

# Rust best practices for MOLT

This Skill guides **target Rust development** and maintenance of MOLT's Rust engine. Use MOLT's migration workflow separately for source-system simplification, behavior baselines, approvals and deployment. **Do not simplify the Rust output by weakening engineering standards**: use appropriate types, modules, explicit contracts, tests and documentation. Avoid unjustified complexity, but don't reject a useful trait, layer or crate solely because it adds code.

## Start with the task

1. Read the affected crate's `Cargo.toml`, relevant modules, tests, and stated toolchain/edition. Read only the references applicable to your task:
   - [Ownership, APIs, errors and unsafe](references/core.md) for new or changed Rust code.
   - [Async, persistence and service boundaries](references/async-data.md) for Axum/Tokio/SQLx, SQLite, concurrency, HTTP or data migration work.
   - [Testing and delivery](references/verification.md) when writing tests or reporting completion.
2. Identify observable behavior, untrusted input, error paths, invariants and performance constraints. For generated Rust, capture the original behavior *before* rewriting. Do not infer business semantics from AST names or file-level imports.
3. Make a coherent, well-structured change. Use idiomatic ownership and `Result` rather than panics on user/repository/network input; add abstractions when they clarify a real boundary or invariant, not as an automatic translation of the source system.
4. Verify the changed behavior and communicate what was **not** validated. Run checks in the appropriate workspace (MOLT itself or the generated app), not blindly in `test_subjects/`.

## Working principles

- Borrow when sufficient; own when state must outlive a scope. Avoid cloning to dodge ownership errors without understanding the data flow.
- Design a clear, stable public API. Model meaningful states and IDs as types when they prevent mistakes; maintainability matters more than minimizing the number of types.
- Use explicit errors with context. Avoid `.unwrap()`/`.expect()` for recoverable conditions; tests and proven internal invariants are different cases.
- Keep `unsafe` absent unless unavoidable. If used, state safety invariants, minimize its scope and test the boundary.
- Treat filenames, imports, generated schemas and performance anecdotes as **evidence to investigate**, not proof. Measure before optimizing. Never set `target-cpu=native` globally for a portable amd64/arm64 artifact.
- Simplify **source** overengineering only after evidence and approval. Design the **target** Rust application to accepted Rust practices and its real functional/operational requirements, including database and native/mobile compatibility; document migration and rollback constraints.

`cargo fmt`, Clippy and tests are gates, not substitutes for contract tests, review or deployment smoke checks. This guidance is intentionally small; consult the official [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/), [Rust Reference](https://doc.rust-lang.org/reference/) and crate docs when an API or language detail matters. The optional third-party `rust-skills` collection is a reference, not a mandated dependency.
