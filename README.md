# MOLT 🦀

*Shedding legacy. Reborn in Rust.* — All code eventually becomes crab.

MOLT is an agent-native toolkit for planning evidence-based simplification and Rust rewrites of legacy full-stack projects. It combines a Rust CLI with a portable [Agent Skill](skills/molt/SKILL.md). The intended stack is Dioxus + Axum + PostgreSQL, with optional Redis and Helm deployment to single-node k3s. **The current CLI is an early file-inventory foundation, not an automatic rewriter or semantic analyzer.**

## Get started

Requires a stable Rust toolchain and Cargo (not bundled).

```sh
cargo build --workspace
cargo run -p molt-cli -- analyze --source /path/to/legacy-repo
cargo run -p molt-cli -- query --source /path/to/legacy-repo project-summary
cargo run -p molt-cli -- query --source /path/to/legacy-repo files --limit 25 --offset 0
cargo test --workspace
```

Install locally with `cargo install --path crates/molt-cli` to use the `molt` command directly. CLI success responses are JSON on stdout; failures are JSON on stderr with nonzero exit status. `analyze` reads source files and writes only `/path/to/legacy-repo/.molt/intelligence.db` (+ SQLite WAL files); add `.molt/` to the **source repository's** `.gitignore`. Hidden/generated directory exclusions are defined in `crates/molt-index/src/lib.rs`. Only `.js`, `.jsx`, `.mjs`, `.cjs`, `.ts`, `.tsx`, `.mts`, `.cts`, `.java` are inventoried; no source contents are stored. The index is rebuildable, so don't store approvals in it.

The Skill guides an agent (Pi, Codex or Claude Code) to inspect code, propose changes, seek approval for risky operations, implement in a separate target tree and verify behavior. See [product baseline and roadmap](docs/product.md) for features that are **not yet implemented**.

License: MIT OR Apache-2.0 (license texts to be added before distribution). Public package-name and trademark availability not yet checked.
