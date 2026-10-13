# MOLT 🦀

*Shedding legacy. Reborn in Rust.* — All code eventually becomes crab.

MOLT is an agent-native toolkit for planning evidence-based simplification and Rust rewrites of legacy full-stack projects. It combines a Rust CLI with a [migration workflow Skill](skills/molt/SKILL.md), a project [Rust best-practices Skill](.agents/skills/rust-best-practices/SKILL.md) and repository [agent instructions](AGENTS.md). The intended stack is Dioxus + Axum + PostgreSQL, with optional Redis and Helm deployment to single-node k3s. **The current CLI indexes file hashes, JS/TS top-level syntax and selected syntax-derived GraphQL/Spring contracts; it is not an automatic rewriter, a Java type checker, or a verified compatibility tool.**

## Get started

Requires a stable Rust toolchain and Cargo (not bundled).

```sh
cargo build --workspace
cargo run -p molt-cli -- analyze --source /path/to/legacy-repo
cargo run -p molt-cli -- query --source /path/to/legacy-repo project-summary
cargo run -p molt-cli -- query --source /path/to/legacy-repo symbols ExampleClass --limit 10
cargo run -p molt-cli -- query --source /path/to/legacy-repo imports src/example.ts
cargo run -p molt-cli -- query --source /path/to/legacy-repo dependents src/example.ts
cargo run -p molt-cli -- query --source /path/to/legacy-repo contracts --category graphql --method mutation --limit 25
cargo run -p molt-cli -- query --source /path/to/legacy-repo contract-issues --limit 25 --offset 0
cargo run -p molt-cli -- query --source /path/to/legacy-repo contracts --category http --route /items/new
cargo run -p molt-cli -- query --source /path/to/legacy-repo http-links --route /items/new
cargo run -p molt-cli -- query --source /path/to/legacy-repo files --limit 25 --offset 0
cargo test --workspace
```

Install locally with `cargo install --path crates/molt-cli` to use the `molt` command directly. CLI success responses are JSON on stdout; failures are JSON on stderr with nonzero exit status. `analyze` reads source files and writes only `/path/to/legacy-repo/.molt/intelligence.db` (+ SQLite WAL files); add `.molt/` to the **source repository's** `.gitignore`. Hidden/generated directory exclusions are defined in `crates/molt-index/src/lib.rs`. Only `.js`, `.jsx`, `.mjs`, `.cjs`, `.ts`, `.tsx`, `.mts`, `.cts`, `.java`, `.graphql` and `.gql` are inventoried; Oxc parses JS/TS and Tree-sitter extracts limited literal Spring routes from Java (`parse_status: unsupported` still means **no Java type analysis**). JS/TS syntax errors report `parse_status: parse_error` and a count, not misleading partial results. Relative imports are conservatively resolved to **indexed JS/TS file paths** (including index files and TS counterparts of `.js` specifiers); non-relative aliases/packages, assets, ambiguous and missing targets remain explicitly unresolved. The resulting edges are not a type-level or verified service dependency graph; only named top-level declarations are indexed. `contracts` extracts literal Spring mapping routes, GraphQL root fields from standalone SDL or non-interpolated `gql`-tagged templates regardless of binding name, and heuristic `outbound_http_candidate` entries (`outbound_http_candidates` in summaries) for literal `this.get/post/put/delete/patch` calls in **any** JS/TS source directory (not necessarily HTTP clients). `contract-issues` lists bounded, source-located reasons for incomplete extraction (`line` may be null or identify only the construct start); index schema v4 upgrades and refreshes older indexes on analyze. `http-links` joins exact HTTP method/path matches with **`path_match_unverified`**; runtime base URLs, auth, resolver-to-API relationships, template paths, package aliases and behavioral parity are not established. No source contents are stored. The index is rebuildable, so don't store approvals in it.

The Skill guides an agent (Pi, Codex or Claude Code) to inspect code, propose changes, seek approval for risky operations, implement in a separate target tree and verify behavior. See [product baseline and roadmap](docs/product.md) for features that are **not yet implemented**, and the [local three-project experiment](docs/test-subjects.md). The subjects' `.molt/` index remains gitignored. See the [isolated local experiment](docs/test-subjects.md) for example source evidence. No sample-specific paths, routes, or schema names are configured in the tool; syntax inventory is **not** a verified rewrite.

License: MIT OR Apache-2.0 (license texts to be added before distribution). Public package-name and trademark availability not yet checked.
