# Migration workflow (target design)

- **Discover:** Inventory JS/TS/JSX/TSX/Java, React, Node, Spring, APIs, databases, external integrations and tests. Current CLI inventories files only. Treat semantic relationships as unknown until verified against source or tests.
- **Simplify:** Present evidence-backed opportunities (duplicate logic, pass-through layers, service consolidation). For every suggestion record origin, rationale, expected behavior, risk, and approval. Do not remove business logic just because a wrapper looks redundant.
- **Rewrite:** Favor small idiomatic Rust modules; planned stack: Dioxus/WASM UI, Axum/Tokio API, SQLx/PostgreSQL, Redis only for reconstructible caching. Media storage in PostgreSQL binary chunks is a design target, not a universal safe default: validate size, streaming, backup and cost.
- **Verify:** Capture baseline first; compare API contracts, integration and browser behavior; run cargo fmt, clippy and tests. Verify database migration, data preservation and rollback. Benchmark only with comparable workloads. Record unsupported features and unknowns.
- **Deploy:** Target single-node k3s with OCI images and Helm, Traefik ingress, a persistent PostgreSQL volume and ephemeral Redis. Confirm target Kubernetes context and require approval. Backup before destructive changes; deployment is verified only after rollout and smoke tests succeed.
- **Resume:** Keep version-controlled plans/decisions separate from the rebuildable SQLite index; record source revision/hash for each task. Do not confuse legacy and generated source indexes.

For now, the CLI implements only inventory and bounded file queries; all other steps are manual agent workflow, not automated features.
