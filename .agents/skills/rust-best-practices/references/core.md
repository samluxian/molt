# Ownership, API and error design

Use these principles for both MOLT crates and generated Rust. They are engineering guidance, not a demand to minimize target Rust code. Match the crate's declared edition and MSRV; look up current crate APIs rather than inventing signatures.

## Ownership and types

- Use `&str`, `&[T]` and `&Path` for read-only inputs when ownership is not needed. Return owned values when data must outlive the input or move to another task. Clone deliberately at ownership boundaries, not reflexively to silence the borrow checker.
- Use `Option<T>` for absence and `Result<T, E>` for failure; define enums/newtypes when they enforce real domain states (e.g. `SourceRevision` versus `AnalysisId`). Don't convert every primitive into a wrapper with no invariant.
- Keep state transitions and cross-layer assumptions visible. Prefer exhaustive matches for domain enums; avoid catch-all arms that mask future cases. Never substitute a default value for an invalid business input without a documented policy.
- Keep module boundaries aligned with actual responsibilities. Public APIs should be documented, cohesive and no broader than needed. A small app need not be a workspace; a larger one may benefit from separate domain, transport and persistence crates when that separation protects behavior and testability.

## Error handling

- User input, disk, network, database, parser and subprocess operations can fail. Return `Result`; propagate with `?` and add location/operation context without logging secrets or entire source contents.
- In a CLI, `anyhow` can assemble failures across libraries. Use `thiserror` or another explicit type when callers must branch on variants. Neither crate is mandatory in every module.
- `.unwrap()` or `.expect()` are acceptable in tests or on a documented, mechanically guaranteed invariant; avoid them for external input. Panics are not JSON diagnostics.
- Distinguish parse failure, unsupported syntax, unresolved reference and verified absence. Do not turn an incomplete analysis into a positive finding. Keep transactional index updates atomic so failure does not erase good prior data.

## Unsafe and performance

- Prefer safe Rust. Before adding `unsafe`, document each invariant and why safe alternatives are insufficient; keep the block small and review/test its boundary. Generated application code should not introduce unsafe merely for speed.
- Profile before adding custom allocators, `SmallVec`, manual SIMD, `Arc<Mutex<_>>`, or deeply generic frameworks. Add a benchmark and compare before/after with comparable workloads.
- Keep distributed binaries portable. Do not set `target-cpu=native` in shared builds intended for k3s on both amd64 and arm64; use target-specific builds only with an explicit deployment plan.

References: [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/), [Rust Reference](https://doc.rust-lang.org/reference/), [Rustonomicon](https://doc.rust-lang.org/nomicon/).
