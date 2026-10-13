# Local migration exercise: `test_subjects/`

The subjects are ignored by Git; do not copy source, `.env*`, credentials or generated SQLite state into this repository. Each directory is an independent checkout. This is an **analysis exercise**, not an approved migration plan.

| Subject | Observed baseline | First compatibility risks to investigate |
|---|---|---|
| `aicardapp/` | React + TypeScript + Vite; Capacitor Android/iOS, Firebase, GraphQL client; Vitest | Native plugins (camera, NFC, contacts, push, filesystem), mobile builds and browser behavior: a Dioxus/WASM page alone cannot replace native bridges. |
| `cardbff/` | TypeScript Apollo Server/Express BFF; GraphQL + subscriptions; Jest | GraphQL schema, websocket/subscription contracts, auth/context and downstream REST calls. |
| `aicardbackend/` | Java 17 Spring Boot/Gradle; MongoDB, Redis, cloud integrations | Do **not** assume a safe automatic move from MongoDB to PostgreSQL. Inventory schema/data, external services and auth before proposing substitutions. |

Run the syntax index without altering any project source:

```sh
cargo run -p molt-cli -- analyze --source test_subjects
cargo run -p molt-cli -- query --source test_subjects project-summary
cargo run -p molt-cli -- query --source test_subjects symbols CardDataSource --limit 10
cargo run -p molt-cli -- query --source test_subjects imports cardbff/src/index.ts --limit 10
cargo run -p molt-cli -- query --source test_subjects dependents cardbff/src/common/environment.ts --limit 10
cargo run -p molt-cli -- query --source test_subjects contracts --category graphql --method mutation --limit 10
cargo run -p molt-cli -- query --source test_subjects contract-issues --limit 10
cargo run -p molt-cli -- query --source test_subjects contracts --category http --route /card/v1/create
cargo run -p molt-cli -- query --source test_subjects http-links --route /card/v1/create
```

Initial run: **1,379** supported files (1,066 JS/TS parsed, 313 Java inventoried; 0 Oxc parse errors). File-level import resolution found 1,274 unambiguous relative JS/TS links; 227 relative imports were not resolved to JS/TS source files (in this sample mainly CSS assets). Package/alias imports are separately marked `non_relative`. With the general-purpose extractors, syntax contract inventory found **161 GraphQL root fields, 169 literal Spring routes, and 108 heuristic `this.<HTTP verb>` candidates**; 98 exact method/path pairs matched as **unverified candidates**. There are **16 TypeScript files with partial contract coverage**; `contract-issues` identifies each as `graphql/interpolated_template` with source path and construct-start line (for example `aicardapp/src/services/graphql/mutation/createCard.ts:9`). These templates cannot be classified conclusively as SDL or client operations by this syntax pass; the tool does not infer contracts from their interpolated bodies. Re-running unchanged yields `changed: 0`. Dynamic/template paths, inherited Spring mappings, custom path aliases, runtime base URLs and resolver call graphs are not fully indexed. Even `contract_issues: 0` would not be a coverage guarantee. These numbers describe this sample, not rules configured in the CLI.

Example candidate to inspect manually (not an approved rewrite): GraphQL `createCard` at `cardbff/src/graphql/modules/card/schema.ts:222`, resolver in `cardbff/src/graphql/modules/card/resolvers.ts:20,27`, literal BFF POST at `cardbff/src/datasources/CardDataSource.ts:30`, and Spring POST at `aicardbackend/src/main/java/com/aile/service/aicard/controller/CardController.java:53`. The `http-links` result establishes **only** the identical literal method/path; resolver wiring and runtime routing require separate verification. This operation writes data, so capture authentication, payload/error, side-effect and rollback baselines before considering it as a migration slice. **No behavior baseline has been executed and no rewrite is approved yet.**

Next work: choose a bounded low-risk flow with existing tests, gather pre-migration behavioral fixtures, resolve dynamic routes and Java types, then propose a migration plan for human review. Review/approval is required for MongoDB migrations, architecture merges and deployment.
