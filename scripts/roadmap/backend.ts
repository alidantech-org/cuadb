import type { GroupSpec } from './types';

// Backend foundation and PostgreSQL correctness work. Tasks are ordered by dependency.
export const backendGroups: GroupSpec[] = [
  {
    key: 'foundation',
    title: '01 — Rust foundation and observability',
    goal: 'Turn the verified application shell into a maintainable Rust service with controlled configuration, predictable errors, observable request behavior and reproducible quality gates. Preserve the current single binary, single Docker runtime and local-only deployment assumptions. This group does not connect to live databases; its deliverables make later adapters safe to introduce without leaking details into HTTP handlers.',
    approach: 'First isolate startup configuration and application state from route code. Then add structured errors, tracing and graceful shutdown so failures are understandable without exposing credentials. Establish a fixture-driven testing pattern and update the root Bun commands only where necessary. Validate each change using formatting, strict Clippy, type checking and isolated HTTP tests. Keep feature work confined to small Rust modules instead of introducing unnecessary crates.',
    risks: 'The main risks are accidental behavior changes to the already-working shell, verbose abstractions that slow development and logs that expose sensitive connection data. Keep defaults backwards compatible and verify that the server works with no database configured. Make absence of a database an explicit disconnected state rather than a startup failure.',
    tasks: [
      {
        key: 'foundation-config',
        title: 'Harden server configuration and startup lifecycle',
        goal: 'Define typed, validated settings for host, port, deadlines, maximum page size and optional connection configuration while preserving the existing zero-credential shell startup. Invalid values should fail fast with actionable, redacted errors instead of panicking or silently selecting dangerous defaults.',
        context: 'Today config only parses a listening host and port. Database credentials, pool size and statement limits will arrive later, so this task should define a minimal configuration boundary without implementing adapters. Environment settings must be documented and secrets never printed in Debug output. Do not force a database connection just to render the shell.',
        checks: [
          'Extract typed configuration validation with explicit defaults, bounds and tests for malformed values.',
          'Verify startup works with no database URL and rejects invalid host, port and timeout values.',
          'Ensure logs and error output cannot print passwords or full credential-bearing URLs.',
          'Document environment variables in .env.example and relevant README sections.',
          'Run native shell and HTTP health smoke checks after the configuration change.'
        ]
      },
      {
        key: 'foundation-errors',
        title: 'Introduce typed application errors and structured request tracing',
        goal: 'Create a consistent error boundary for database, validation, rendering and transient infrastructure failures. Operators should diagnose problems from safe logs while browsers receive stable status codes and understandable HTML responses without stack traces.',
        context: 'The current web handler prints template errors and returns a bare 500 response. Later HTTP routes will have user-supplied identifiers, database timeouts and catalog permission errors. Centralizing error conversion early prevents each handler from inventing incompatible behavior. Tracing must avoid query parameters containing secrets, full connection URLs and cell content.',
        checks: [
          'Define a focused application error type and HTTP status mapping with no sensitive debug text.',
          'Integrate structured request IDs and minimal redacted tracing for route-level diagnostics.',
          'Add tests for bad request, unavailable database, timeout and template failure mappings.',
          'Keep health and initial page routes working with existing success behavior.',
          'Document when to log an internal cause and when to expose a user-safe message.'
        ],
        dependsOn: ['foundation-config']
      },
      {
        key: 'foundation-lifecycle',
        title: 'Implement graceful shutdown and bounded request behavior',
        goal: 'Ensure CuaDB exits cleanly, closes outstanding work and does not hang indefinitely during HTTP or database operations. Keep the service resource footprint predictable for laptop use and single-container deployment.',
        context: 'A local-only tool can still encounter stalled network connections, abandoned browser requests and repeated reloads. Rust cancellation, timeout defaults and an orderly shutdown path should be designed before adding real SQL operations. The health endpoint must remain a shallow server signal and should not issue expensive queries or expose secrets.',
        checks: [
          'Handle process termination signals and let the Axum server shut down gracefully.',
          'Define request concurrency, body-size and timeout policies with realistic defaults.',
          'Add cancellation behavior tests that do not require a database or network dependency.',
          'Confirm /health remains fast and reports server liveness rather than query success.',
          'Document shutdown expectations for both bun run dev and Docker.'
        ],
        dependsOn: ['foundation-errors']
      },
      {
        key: 'foundation-quality',
        title: 'Make all root quality commands reproducible and testable',
        goal: 'Make formatting, checking, unit tests, asset compilation and Docker image verification consistent across Fedora development and fresh clones. Keep Bun as a task runner rather than introducing a second application server.',
        context: 'The shell has passed local development tests. Cargo.lock and bun.lock are now present in the repository, and the toolchain should honor them. Add focused test helpers and static checks only when they improve confidence. Distinguish tasks requiring a live PostgreSQL fixture from those that can run offline in every developer environment.',
        checks: [
          'Verify root dev, check, build, start, format and test scripts with documented prerequisites.',
          'Ensure Rust, Bun and Docker builds prefer committed lockfiles and fail visibly on mismatch.',
          'Add a dedicated smoke-test path for HTTP routes and generated static assets.',
          'Document fixture requirements without requiring a database to run ordinary unit tests.',
          'Capture a clean reproducible command sequence in README and developer documentation.'
        ],
        dependsOn: ['foundation-lifecycle']
      }
    ]
  },
  {
    key: 'contracts',
    title: '02 — Secure database contracts and connections',
    goal: 'Provide one small engine-independent interface for database discovery, metadata and row retrieval. Make a PostgreSQL URL the default connection input and define capabilities before expanding to MySQL, SQLite or SQL Server. This group owns lifecycle, timeout and read-only guarantees; it must not leak SQL or vendor-specific catalog assumptions into web templates.',
    approach: 'Model connection identities, schema objects, table columns, row cells, pagination parameters and feature flags as typed Rust data. Then add URL recognition, pooling, TLS policy, safe error handling and read-only session enforcement in the PostgreSQL implementation. Use an explicit adapter dispatch that can later support engines with different catalog behavior. Introduce connection timeouts and bounded pool sizes before exposing any browser-controlled navigation.',
    risks: 'A read-only UI is not a read-only database account. The adapter must never offer arbitrary SQL, and local credentials should use a least-privilege role. URL parsing and metadata enumeration must not let an untrusted browser request open new arbitrary network targets. Connection information should not be stored in browser HTML, history, query parameters or logs.',
    tasks: [
      {
        key: 'contracts-types',
        title: 'Define engine-neutral objects, capabilities and error types',
        goal: 'Establish minimal data contracts for connections, namespaces, relations, columns, keys, statistics, pages and typed cell values. The interface must expose unavailable capabilities explicitly instead of pretending that all SQL engines implement the same metadata.',
        context: 'PostgreSQL exposes schemas and a rich catalog, MySQL commonly represents databases as namespaces, and SQLite is file-backed. These differences should be represented behind a focused adapter contract so the templates can display a consistent explorer. Avoid generic ORM layers, user-generated SQL and premature support code for unavailable drivers.',
        checks: [
          'Define typed relation identifiers and feature flags without engine-specific catalog leakage.',
          'Specify paged-row and metadata response models with explicit unavailable-statistic markers.',
          'Create adapter boundary tests using a fake or stub implementation.',
          'Document semantics of namespace and database switching across supported engines.',
          'Keep compilation independent of a running PostgreSQL server.'
        ],
        dependsOn: ['foundation-errors']
      },
      {
        key: 'contracts-url',
        title: 'Parse and validate PostgreSQL connection URLs safely',
        goal: 'Accept postgresql:// and postgres:// URLs through server-side configuration and validate malformed input, unsupported schemes, network locations and TLS options. A user should be able to point CuaDB at its existing postgis container without disclosing credentials to the browser.',
        context: 'The Fedora setup currently publishes postgis on host loopback; Docker-to-Docker access should instead use a shared network and postgis:5432. URL parsing must respect percent-encoding, database names and connection parameters. For the initial milestone, the URL is trusted operator configuration, not a freely editable browser-submitted URL.',
        checks: [
          'Validate supported schemes, database names, URL encoding and disallowed unsupported options.',
          'Load a configured connection without leaking password or user information to HTML/log output.',
          'Document host, Docker network, TLS and credentials handling for PostgreSQL 18.',
          'Add tests for malformed URLs, redaction and absent optional configuration.',
          'Keep any future browser connection form behind explicit permission and security design.'
        ],
        dependsOn: ['contracts-types','foundation-config']
      },
      {
        key: 'contracts-pool',
        title: 'Implement bounded PostgreSQL pool and connection lifecycle',
        goal: 'Add asynchronous PostgreSQL connections using SQLx, with bounded pool size, acquisition and connection timeouts, predictable retry behavior and explicit startup status. Avoid reconnecting on every HTMX request or creating unbounded background tasks.',
        context: 'The application should start in disconnected mode if no URL is supplied. If a URL exists, failed connections must produce an informative, redacted state rather than crashing or hanging the UI. Network and TLS settings need clear boundaries. Connection state should be shareable from Axum application state without mutable global variables.',
        checks: [
          'Create one lazily initialized or bounded connection pool for the configured database.',
          'Apply connection, acquisition and query timeout defaults with configurable upper bounds.',
          'Distinguish disabled, connecting, connected and failed states in a testable model.',
          'Verify retry and failure behavior without exposing raw driver errors or credentials.',
          'Add a live PostgreSQL fixture test behind an explicit environment gate.'
        ],
        dependsOn: ['contracts-url']
      },
      {
        key: 'contracts-readonly',
        title: 'Enforce read-only database access and least-privilege assumptions',
        goal: 'Make it materially difficult for a bug or browser request to mutate a connected database. Every supported query path must be constrained to introspection and SELECT operations, using least-privilege credentials and transaction/session limits as additional safeguards.',
        context: 'Disabling buttons is not a security boundary. PostgreSQL read-only transaction defaults and a dedicated SELECT-only role should be considered separately; catalog and statistics permissions may vary. Views and security-definer functions require care, and read-only transactions do not replace a properly restricted database account.',
        checks: [
          'Document a least-privilege PostgreSQL role with schema and table read permissions.',
          'Enforce read-only transaction/session behavior where the driver and engine permit it.',
          'Reject unsupported statement types and avoid any user-controlled SQL execution endpoint.',
          'Test that error paths and connection lifecycle cannot silently enable writes.',
          'Explain how RLS, denied tables and metadata visibility appear to the browser.'
        ],
        dependsOn: ['contracts-pool']
      },
      {
        key: 'contracts-state',
        title: 'Expose safe connection status and operator diagnostics',
        goal: 'Provide browser-safe connection health and meaningful failure categories without embedding secrets, stack traces or SQL statements in rendered templates. Separate server liveness from actual database reachability to keep the app usable when its database is offline.',
        context: 'The existing /health endpoint deliberately checks only the Rust service. A distinct connection-status route can indicate supported driver, database name when safe, reachability and whether credentials are missing. It must not publish raw connection URLs or grant visitors the ability to scan arbitrary hosts.',
        checks: [
          'Return safe status categories such as disconnected, ready, unavailable and insufficient permissions.',
          'Keep /health independent of database availability and fast under network failure.',
          'Use redacted operator logs with stable error codes for troubleshooting.',
          'Test no configured URL, wrong password, denied schema and temporary network loss.',
          'Integrate the status contract with a future HTMX connection panel.'
        ],
        dependsOn: ['contracts-readonly']
      }
    ]
  },
  {
    key: 'postgres-catalog',
    title: '03 — PostgreSQL schemas, relations and metadata',
    goal: 'Deliver complete, permission-aware catalog browsing for PostgreSQL and PostGIS installations. Users must find schemas, tables and views; inspect column definitions, indexes, keys, constraints and storage sizes; and understand when a statistic is approximate or unavailable. All catalog queries must be scoped, efficient and read-only.',
    approach: 'Use PostgreSQL pg_catalog rather than building the interface around unportable information_schema-only assumptions. Filter system objects unless explicitly requested, honor table and schema permissions, and represent relations and partition parents accurately. Structure and statistics queries should be small, parameterized where possible and bounded. Add fixtures covering mixed-case names, Unicode, unusual types, foreign keys and partitions.',
    risks: 'Catalog APIs differ by server version and enabled extensions. Approximate row counts are not exact, partitioned parents may have little physical storage, and statistics can be stale. Never issue a full-table COUNT automatically. An unavailable or permission-denied metric must be labeled honestly instead of fabricated.',
    tasks: [
      {
        key: 'catalog-navigation',
        title: 'Discover accessible PostgreSQL schemas, tables and views',
        goal: 'Return a stable tree of visible schemas and supported relation types for the sidebar, including ordinary tables, views, materialized views and partitions. Hide noise by default while retaining a clear path to inspect relevant non-default schemas.',
        context: 'PostgreSQL URLs connect to one database, not the whole server. Catalog discovery should use pg_namespace, pg_class and ownership/privilege checks. Sorting and filtering should be stable; tables with unusual Unicode or mixed-case names must remain navigable without losing their exact identifier spelling.',
        checks: [
          'List accessible user schemas without assuming public is the only namespace.',
          'Discover tables, views, materialized views and partitioned parents with correct types.',
          'Sort and filter names safely, preserving exact identifiers and handling empty schemas.',
          'Test denied permissions, quoted names, system schemas and table type distinctions.',
          'Expose stable typed objects through the adapter instead of direct template SQL.'
        ],
        dependsOn: ['contracts-readonly']
      },
      {
        key: 'catalog-columns',
        title: 'Inspect PostgreSQL columns, types, defaults and constraints',
        goal: 'Provide a complete column structure model including native type, nullability, default expression, generated/identity status and primary or unique key indicators. The user should understand table shape without opening a SQL console.',
        context: 'PostgreSQL uses catalog metadata for arrays, domains, enums and extension types. Avoid flattening all types into a generic string that loses meaningful information. The UI should receive safe display-ready metadata, while backend representation retains stable identifiers for later sort and cell decoding.',
        checks: [
          'Return ordered column names, data types, nullable flags and default expressions.',
          'Identify primary keys, identity/generated columns, domains and enum information.',
          'Handle views and atypical extension types without panics or false metadata.',
          'Test composite and mixed-case identifiers against PostgreSQL fixture tables.',
          'Label unsupported details explicitly rather than returning empty invented values.'
        ],
        dependsOn: ['catalog-navigation']
      },
      {
        key: 'catalog-relations',
        title: 'Expose keys, foreign keys, indexes and partition hierarchy',
        goal: 'Model PostgreSQL relationships and indexes so users can understand table references, uniqueness and sorting opportunities. Keep this metadata read-only and bounded, and support partitioned tables without double-counting data or misrepresenting inherited structure.',
        context: 'Primary keys and usable unique indexes are important for stable row pagination. Foreign keys may cross schemas, and index details may include expressions or predicates. The adapter should expose useful human-readable metadata and preserve links to related objects for later UI navigation.',
        checks: [
          'Discover foreign key source and target columns with schema-qualified relation names.',
          'Return index names, uniqueness, primary status and relevant indexed columns.',
          'Represent partition parents and child tables with unambiguous relationships.',
          'Test composite keys, expression indexes and links to inaccessible related tables.',
          'Provide metadata usable by both Structure view and pagination planning.'
        ],
        dependsOn: ['catalog-columns']
      },
      {
        key: 'catalog-stats',
        title: 'Implement efficient PostgreSQL table size and statistics inspection',
        goal: 'Expose total relation storage, table bytes, index bytes and estimated rows without running expensive full scans. Make estimates clearly distinguishable from exact measurements and avoid misleading totals for partitioned or foreign tables.',
        context: 'PostgreSQL provides pg_total_relation_size, pg_table_size, pg_indexes_size and pg_stat* catalog views. Partitioned parents, empty tables and un-analyzed relations need special handling. Displaying estimated row counts from catalog statistics is preferable to forcing COUNT(*) on large production datasets.',
        checks: [
          'Report available table, index and total size metrics with correct units and nullability.',
          'Return estimated row count with an explicit approximate indicator.',
          'Handle partitions, materialized views and foreign relations without false totals.',
          'Avoid automatic exact row counts and bound metadata query execution times.',
          'Verify expected metrics on populated, empty and unanalyzed fixture tables.'
        ],
        dependsOn: ['catalog-relations']
      },
      {
        key: 'catalog-compat',
        title: 'Validate catalog coverage against PostgreSQL 18 and PostGIS',
        goal: 'Prove the catalog adapter is usable with the existing Fedora postgis/postgis:18-3.6 image and conventional PostgreSQL deployments. Record supported object behavior and failure modes without depending on a developer machine for ordinary unit tests.',
        context: 'The current target is PostgreSQL 18 with a PostGIS extension available, but compatibility must not depend on geography tables existing. Live integration tests should use opt-in fixture URLs and a dedicated disposable test schema or database. Never run destructive test setup on the user’s production database.',
        checks: [
          'Create repeatable integration fixtures for tables, views, indexes, constraints and partitions.',
          'Run opt-in tests with a disposable PostgreSQL instance and least-privilege reader.',
          'Verify catalog behavior on a PostGIS-enabled database and one without extensions.',
          'Document version assumptions, unsupported objects and permission-dependent outcomes.',
          'Keep local unit tests passing when PostgreSQL is not installed.'
        ],
        dependsOn: ['catalog-stats']
      }
    ]
  },
  {
    key: 'postgres-rows',
    title: '04 — Safe data retrieval, sorting and pagination',
    goal: 'Implement the core read-only viewing capability with bounded, predictable SQL and correct typed values. A user must be able to open any permitted PostgreSQL table, read a page, sort an allowed column and navigate safely without requesting the entire relation. This group is the performance and query-security center of the project.',
    approach: 'Use identifier validation against discovered catalog metadata, engine-specific quoting and bound query values. Start with deterministic ORDER BY and limited OFFSET pagination, then add keyset navigation when a suitable unique index exists. Treat NULL ordering and changing data explicitly. Decode cells into bounded typed representations and never allow browser-supplied raw SQL.',
    risks: 'Dynamic schema and column names cannot be used as bind parameters, so direct string concatenation is unacceptable. Unindexed sorts and deep OFFSET scans can be expensive; enforce limits and consider timeouts. A table without a unique key cannot guarantee consistent paging when records change. Browser output must escape all returned text.',
    tasks: [
      {
        key: 'rows-sql-safe',
        title: 'Build validated identifier and query construction helpers',
        goal: 'Ensure every dynamic SQL identifier comes from a catalog-validated allowlist, with correct PostgreSQL quoting and no execution of user-submitted statements. This provides the trusted boundary for relation selection, sorting and row reads.',
        context: 'SQLx parameter binding protects values but not identifiers such as schema, table and column names. Catalog-discovered relation IDs or validated exact names should be used, with rules for reserved words, Unicode, quotes and case sensitivity. The helper should be small, independently testable and used by all PostgreSQL row routes.',
        checks: [
          'Implement safe schema/table/column resolution against trusted catalog objects.',
          'Quote identifiers correctly and bind pagination values instead of concatenating them.',
          'Reject unknown columns, unsupported relation types and malformed sort instructions.',
          'Test injection payloads, quoted names, unusual Unicode and reserved keywords.',
          'Require all row-fetch code paths to use the shared validated helper.'
        ],
        dependsOn: ['catalog-columns']
      },
      {
        key: 'rows-first-page',
        title: 'Fetch bounded PostgreSQL row pages with deterministic ordering',
        goal: 'Return a small set of records for a permitted relation without loading its full contents or computing exact totals. Preserve a stable column order and make explicit when the relation lacks a reliable key for deterministic navigation.',
        context: 'The grid defaults to fifty records per page and must cap requested page size at a configured maximum. PostgreSQL query results may contain NULL, large TOAST values and extension types. The first iteration should favor correctness, read-only enforcement and predictable failure handling over premature optimization.',
        checks: [
          'Fetch only requested columns and a bounded number of rows using stable ordering where possible.',
          'Apply statement timeouts and max page size before submitting SQL to PostgreSQL.',
          'Represent empty tables, permission errors and unsupported types without crashes.',
          'Add tests for 0, 1, 50 and more than 50 rows with deterministic order assertions.',
          'Record safe slow-query diagnostics without logging cell values or secrets.'
        ],
        dependsOn: ['rows-sql-safe','contracts-pool']
      },
      {
        key: 'rows-sort',
        title: 'Support single-column sorting with safe null and tie behavior',
        goal: 'Allow ascending and descending sorting on valid columns while preserving deterministic ties where feasible. Make sorting parameters explicit and ensure the UI cannot request arbitrary expressions or issue unbounded database work.',
        context: 'PostgreSQL exposes NULLS FIRST/LAST behavior and case-sensitive identifiers. To avoid duplicates across page boundaries, a unique key may be added as a tie-breaker when compatible. For unsupported or expensive sorts, return a clear user-facing explanation instead of silently sorting data in the browser.',
        checks: [
          'Accept only catalog-listed columns and an explicit ascending or descending direction.',
          'Define NULL ordering and unique tie-breaking behavior with repeatable SQL generation.',
          'Test mixed NULL values, duplicate sort values, text and timestamp columns.',
          'Enforce row-size limits, query timeout and rejected unsupported sort requests.',
          'Document behavior when an index is unavailable or ordering cannot be stable.'
        ],
        dependsOn: ['rows-first-page','catalog-relations']
      },
      {
        key: 'rows-pagination',
        title: 'Implement previous/next pages and indexed cursor optimization',
        goal: 'Provide predictable page navigation using bounded offsets for ordinary tables and keyset paging when a suitable index and sort order exist. Keep page tokens opaque, validated and scoped to the chosen relation and sort.',
        context: 'Deep OFFSET is increasingly expensive on large tables. Cursor-based reads can improve performance but require a stable unique ordering and careful handling of NULL values. This task should introduce an explicit pagination strategy rather than promising keyset navigation on tables that cannot support it.',
        checks: [
          'Implement previous/next states and safe bounds for requested page numbers or cursors.',
          'Choose keyset pagination only when a suitable unique key and ordering are available.',
          'Validate or sign opaque cursors to prevent alteration across relations or sorts.',
          'Test empty, last-page, duplicate-key, NULL and changing-row scenarios.',
          'Document consistency limitations when concurrent database writes occur.'
        ],
        dependsOn: ['rows-sort']
      },
      {
        key: 'rows-cell-types',
        title: 'Normalize PostgreSQL cell types and bound large values',
        goal: 'Convert supported SQLx row values into a consistent typed data model for Askama templates while retaining nullability and avoiding unlimited memory or HTML response sizes. Support graceful representation of extension and unknown types.',
        context: 'PostgreSQL tables contain primitives, UUIDs, timestamps, JSON, arrays, bytea and custom enums. The grid does not need to materialize every value in full; long content should be truncated with a safe marker and inspected on demand through a separate bounded endpoint. Output escaping is mandatory.',
        checks: [
          'Decode common PostgreSQL scalar types and NULL distinctly from empty strings.',
          'Cap rendered bytes per cell, total response size and expanded-value retrieval.',
          'Preserve precise raw representations for timestamps, numbers and UUID values.',
          'Handle unsupported or extension types with a safe textual fallback and type indicator.',
          'Test UTF-8 escaping, very large values, binary data and malformed conversions.'
        ],
        dependsOn: ['rows-first-page']
      }
    ]
  }
];
