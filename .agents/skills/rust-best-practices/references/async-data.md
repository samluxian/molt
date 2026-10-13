# Async, persistence and application boundaries

Load for Tokio/Axum, SQLx, SQLite indexing, HTTP, concurrency or data-migration tasks. Choosing target Rust best practices does **not** authorize changing source-system behavior or storage semantics.

## Async and concurrency

- Use async when operations must overlap; synchronous code is appropriate for a synchronous CLI. Never block an async executor with expensive parsing, filesystem walks or CPU work: use a controlled blocking worker when needed. `tokio::fs` may wrap blocking I/O; benchmark many-small-file workloads rather than assuming it is faster.
- Set explicit timeouts, cancellation and concurrency bounds for network calls and long-running jobs. Do not hold a synchronous mutex or database transaction across `.await`. Prefer immutable sharing or message passing when ownership is clearer.
- Propagate errors and preserve source evidence. Handle partial failure: distinguish retryable I/O from malformed input or unsupported constructs. Do not mark a run `completed` until its index transaction is committed.

## HTTP and business contracts

- At the Axum boundary, validate and deserialize untrusted requests; preserve status codes, error shape, authorization, headers and pagination where compatibility requires them. Keep domain decisions out of transport handlers when doing so aids testing; use concrete types unless polymorphism is justified.
- Treat GraphQL schemas, REST routes, mobile/native bridge behavior and external integration as distinct contracts. Syntax-level endpoint matches are candidate evidence, not a verified BFF-to-backend call graph.
- Keep user-visible errors structured and safe. Redact credentials, tokens and personally identifiable data from logs and diagnostics.

## Persistence

- For MOLT's rebuildable SQLite index, use parameterized SQL, explicit schema migrations, transactions, indexes for actual queries and bounded/paginated output. Schema changes must upgrade existing `.molt/intelligence.db` files safely; stale edges must be removed or invalidated when source changes.
- For a target application, model database errors, transaction scope, data ownership, schema versioning, backup and restore. PostgreSQL/SQLx is a product target, not proof that a MongoDB document model or object store can be replaced without behavior and data loss. Assess data volume, consistency, media streaming and external dependencies before deciding.
- Migration steps affecting data, storage or deployment require a reviewed plan, rollback strategy and explicit user approval. Never run a destructive migration on the test subjects as an incidental test.

References: [Tokio documentation](https://docs.rs/tokio/), [Axum documentation](https://docs.rs/axum/), [SQLx documentation](https://docs.rs/sqlx/), [SQLite transactions](https://www.sqlite.org/lang_transaction.html).
