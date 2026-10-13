# Testing and delivery gates

Load when writing tests, reviewing a Rust change or reporting progress. Target Rust must meet good engineering standards **and** the migration's separately established behavioral baseline.

## Test the contract, not just the happy path

- Add focused Rust unit tests for parsing, validation, domain invariants and failure paths. Add integration tests for CLI JSON/exit behavior, SQLite migrations, stale-record removal and target application interfaces. Use temporary fixtures rather than coupling CI to ignored `test_subjects/`.
- Use table-driven or property-based tests where useful, not as a ritual. Test ambiguity, unsupported syntax, changed/deleted source, bounded queries, Unicode/line positions and transaction rollback when touching the index.
- For a Rust rewrite, collect **pre-migration** examples/fixtures of observable behavior first. Verify GraphQL/REST contracts, authentication, database effects, mobile-facing flows and regressions on the generated app. If baseline infrastructure is unavailable, report that as an evidence gap; don't mark compatibility verified.
- When appropriate, use browser tests, integration environments and measured benchmarks; passing `cargo test` alone proves neither functional parity nor deployment success.

## Local checks

Run in the workspace that was actually changed:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Format with `cargo fmt --all` before the format check if needed. Keep CI reproducible; pin dependencies with `Cargo.lock` for applications/CLIs. If checks fail (including pre-existing failures), capture the relevant error and state clearly what did and did not pass. Never turn off lints or delete tests merely to get a green report.

## Release and deployment

- Do not claim a benchmark without a comparable workload, or a deploy without rollout and smoke tests. Document unsupported features, risks, approvals and exact verification commands/results.
- Check packaged binaries for the intended architectures and check migrations plus rollback/restore on disposable data. Never make k3s context, credentials, or deployment a side effect of running tests.

References: [Cargo tests](https://doc.rust-lang.org/cargo/commands/cargo-test.html), [Clippy](https://doc.rust-lang.org/clippy/), [Rust API Guidelines—documentation](https://rust-lang.github.io/api-guidelines/documentation.html).
