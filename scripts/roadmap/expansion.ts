import type { GroupSpec } from './types';

// Multi-engine support and release work intentionally follows a usable PostgreSQL MVP.
export const expansionGroups: GroupSpec[] = [
  {
    key: 'mysql',
    title: '09 — MySQL and MariaDB compatibility',
    goal: 'Bring MySQL and MariaDB into CuaDB through a separate adapter while preserving the same read-only navigation and data-grid contract. Reuse common models and browser components, but implement engine-specific discovery, identifier quoting, data decoding and statistics rather than assuming PostgreSQL SQL syntax works everywhere.',
    approach:
      'Implement URL recognition and bounded SQLx MySQL pools, then map information_schema and vendor-specific metadata into the capability contract. Reuse the shared pagination and rendering models where correct, adding MySQL-specific sorting and type handling. Validate on independently provisioned disposable MySQL and MariaDB fixtures. Avoid broad admin features such as server variables, permissions editing or SQL console support.',
    risks:
      'MySQL and MariaDB differ in metadata details, collations, server modes and some data types; even identical URL schemes do not guarantee identical behavior. INFORMATION_SCHEMA statistics can be approximate, some storage engines report different physical sizes, and row ordering without a key remains unstable. Make absent metrics explicit.',
    tasks: [
      {
        key: 'mysql-connect',
        title: 'Implement MySQL/MariaDB URL parsing, pools and capabilities',
        goal: 'Support mysql:// connections through a bounded, read-only adapter that identifies the server flavor and reports available catalog, data and statistics features. Keep network failure and authentication errors consistent with PostgreSQL behavior.',
        context:
          'MySQL and MariaDB are common application databases, but sharing a driver does not make them equivalent in every metadata capability. A user should supply an operator-controlled URL or a saved trusted profile, without browser-accessible credentials. Testing should run against disposable fixtures rather than a developer’s personal database.',
        checks: [
          'Recognize supported MySQL URLs and reject unsupported option or host forms.',
          'Create bounded connection pools with redacted errors and configurable timeouts.',
          'Expose engine/flavor capabilities to the common UI contract.',
          'Enforce least-privilege read-only access as far as the engine supports it.',
          'Add independent MySQL and MariaDB connection failure integration tests.',
        ],
        dependsOn: ['contracts-types', 'contracts-readonly'],
      },
      {
        key: 'mysql-catalog',
        title: 'Discover MySQL/MariaDB namespaces, relations and columns',
        goal: 'Map engine catalog metadata into the common database explorer tree. Users should browse available databases, tables and views, then inspect column types, defaults, indexes, primary keys and foreign-key references without a SQL editor.',
        context:
          'MySQL typically treats a database as the namespace equivalent; do not present fictitious PostgreSQL-style nested schemas. Preserve exact identifier case where the server permits it and account for collation and filesystem case-sensitivity differences. Metadata must reflect the connected role’s visibility.',
        checks: [
          'List accessible databases, tables and views with engine-correct object types.',
          'Return columns, defaults, nullable flags, primary keys and foreign-key relationships.',
          'Handle identifier quoting and case behavior without SQL injection risk.',
          'Report unsupported metadata capability rather than fabricating fields.',
          'Test mixed-case, empty and permission-restricted fixture databases.',
        ],
        dependsOn: ['mysql-connect'],
      },
      {
        key: 'mysql-rows',
        title: 'Fetch and paginate MySQL/MariaDB rows with typed values',
        goal: 'Provide the same bounded grid and sort interaction used for PostgreSQL, with MySQL-specific SQL generation, type decoding and deterministic-order rules. Avoid front-end forks for the two database families.',
        context:
          'MySQL date, decimal, JSON, binary and enum representations can differ from PostgreSQL. Limit retrievals, use safe identifiers and bind ordinary values. Cursor paging should be used only when a valid stable unique key and compatible ordering exist; otherwise disclose paging limitations.',
        checks: [
          'Implement allowlisted relation and column quoting for MySQL SQL generation.',
          'Provide bounded pages, safe sort controls and deterministic ties when available.',
          'Normalize JSON, binary, decimal, time and NULL values without losing meaning.',
          'Reuse shared page responses and HTMX templates without MySQL-specific HTML branches.',
          'Test pagination and sorting against MySQL and MariaDB fixtures.',
        ],
        dependsOn: ['mysql-catalog', 'rows-pagination'],
      },
      {
        key: 'mysql-stats',
        title: 'Map MySQL/MariaDB sizes and metadata limitations to Info tab',
        goal: 'Display accurate available metadata such as estimated rows, storage and index bytes while making engine-specific limitations visible. The UI should not pretend exact PostgreSQL-equivalent statistics are always available.',
        context:
          'Storage engines and server versions can report estimates differently, and INFORMATION_SCHEMA values may be stale or missing. Table and index sizes must be labeled according to the underlying engine semantics. The core capability interface should make these distinctions explicit without special-casing templates.',
        checks: [
          'Read supported table and index size metrics without costly full-table scans.',
          'Label approximate row counts and unsupported storage values honestly.',
          'Verify behavior with views, empty tables and different storage engines where available.',
          'Keep the Structure and Info tabs reusable across PostgreSQL and MySQL adapters.',
          'Document MySQL/MariaDB-specific caveats and validated database versions.',
        ],
        dependsOn: ['mysql-rows', 'inspect-info'],
      },
    ],
  },
  {
    key: 'sqlite',
    title: '10 — SQLite file-based database adapter',
    goal: 'Enable browsing local SQLite database files securely from the same CuaDB interface. A database file must be mounted into the container or available through a configured host path; the viewer may inspect it but must never edit the database or produce write side-effects as part of ordinary browsing.',
    approach:
      'Treat SQLite as a local file source rather than a network server. Define allowed file path roots and open files read-only, using sqlite_master/sqlite_schema and PRAGMA metadata to discover tables, views, columns and foreign keys. Reuse common row/pagination models while documenting SQLite’s dynamic typing and limited storage statistics.',
    risks:
      'User-controlled arbitrary file paths would expose sensitive host files, and SQLite file modes may cause unexpected writes or WAL/locking interactions if misconfigured. The Docker image should only read explicitly mounted paths, preferably read-only. SQLite has no PostgreSQL-style server schemas or equivalent per-table storage sizes.',
    tasks: [
      {
        key: 'sqlite-connect',
        title: 'Open mounted SQLite databases through restricted read-only paths',
        goal: 'Accept a configured SQLite URL or file path only within an allowed mounted directory and open the database in read-only mode. Reject arbitrary filesystem traversal, unsupported URI options and paths outside configured roots.',
        context:
          'SQLite differs from network databases because the file is the database. A local web viewer must not become a general file browser for the host or container. Enforce canonical path checks, reject symlink escape where feasible, and document read-only mount patterns for Docker and native development.',
        checks: [
          'Validate configured SQLite URLs against an explicit allowed data root.',
          'Open the file without creating a new database or allowing modifications.',
          'Test traversal attempts, missing files, symlinks and permission failures.',
          'Document read-only Docker bind mounts and limitations with WAL databases.',
          'Keep SQLite file paths and sensitive filesystem details out of the browser.',
        ],
        dependsOn: ['contracts-types', 'contracts-readonly'],
      },
      {
        key: 'sqlite-catalog',
        title: 'Discover SQLite tables, views, columns and key constraints',
        goal: 'Map SQLite schema objects into the common explorer without inventing server databases or PostgreSQL namespaces. Show columns, declared types, primary and foreign keys and indexes where SQLite metadata supports them.',
        context:
          'SQLite uses sqlite_schema and PRAGMA calls for metadata. Dynamic typing, WITHOUT ROWID tables and virtual tables need careful handling. An attached database namespace should be shown only when deliberately supported and not imply that arbitrary ATTACH statements can be sent through the browser.',
        checks: [
          'List ordinary tables, supported views and virtual tables with accurate type distinctions.',
          'Return columns, declared types, primary keys, foreign keys and indexes.',
          'Handle WITHOUT ROWID and generated columns without assuming rowid exists.',
          'Avoid introducing an arbitrary SQL, ATTACH or schema-modification endpoint.',
          'Test unusual SQLite names and empty database files with disposable fixtures.',
        ],
        dependsOn: ['sqlite-connect'],
      },
      {
        key: 'sqlite-rows',
        title: 'Provide safe SQLite row pagination and dynamic type rendering',
        goal: 'Read bounded pages from SQLite files with stable sorting where possible, using engine-correct identifier quoting and row identity rules. Preserve the common grid interface while accurately representing SQLite’s dynamic per-value types.',
        context:
          'SQLite permits varying storage classes within a column and may use rowid or an explicit primary key depending on table shape. Query generation must not assume PostgreSQL syntax, and sorting should be deterministic only when a unique key can be established. Expanded values remain size-limited and HTML-escaped.',
        checks: [
          'Implement validated table and column resolution with safe SQLite identifier quoting.',
          'Retrieve bounded pages with clear first/last states and sorting controls.',
          'Handle rowid, WITHOUT ROWID and unstable ordering explicitly.',
          'Display SQLite storage classes including NULL, integer, real, text and blob.',
          'Test large blobs, mixed-typed columns and readonly connection behavior.',
        ],
        dependsOn: ['sqlite-catalog', 'rows-pagination'],
      },
      {
        key: 'sqlite-info',
        title: 'Expose SQLite capabilities and integration tests honestly',
        goal: 'Complete SQLite support with verified Info and Structure behavior that clearly describes unavailable per-table size estimates. Ensure that read-only operation works with real mounted files in Docker as well as native development.',
        context:
          'The SQLite database file has a measurable file size, but accurate data-versus-index storage per relation may be unavailable without optional extensions. Surface known file-level metadata instead of manufacturing PostgreSQL-equivalent numbers. Test fresh fixtures and locked or WAL-mode databases carefully.',
        checks: [
          'Render usable SQLite database information with explicit unavailable metrics.',
          'Test in-memory or disposable fixture files across relevant table types.',
          'Verify Docker bind mounts read-only and no write side effects after browsing.',
          'Document SQLite path rules, WAL caveats and limits of available storage statistics.',
          'Run common adapter contract tests across PostgreSQL, MySQL and SQLite.',
        ],
        dependsOn: ['sqlite-rows', 'inspect-info'],
      },
    ],
  },
  {
    key: 'profiles',
    title: '11 — Connection profiles and everyday usability',
    goal: 'Allow safe switching among multiple known database connections while keeping credentials out of the browser and eliminating the need to repeatedly reconfigure the viewer. Improve the everyday explorer experience with durable, non-sensitive preferences and sensible keyboard shortcuts.',
    approach:
      'Start with server-owned named profiles configured through environment or mounted configuration. Present labels, engine and reachability, not raw URLs. Only after a security review consider allowing browser creation or modification of profiles. Keep non-secret preferences such as selected theme or page size local, while ensuring no row contents or credentials are persisted.',
    risks:
      'Browser-managed connections can become an SSRF or secret-exposure mechanism if implemented casually. Profiles should be controlled by the operator in the first release, and persistent credentials require an explicit storage and encryption design. Local browser history or settings must not cache raw database content.',
    tasks: [
      {
        key: 'profiles-config',
        title: 'Support multiple trusted server-side database profiles',
        goal: 'Let a single CuaDB instance choose among more than one configured connection, including different PostgreSQL databases and supported other engines. Keep profile labels safe and never expose connection URLs or passwords through the HTML interface.',
        context:
          'A PostgreSQL URL selects one database, so browsing other databases on the same server requires a separate connection. Multi-profile configuration should remain optional and backwards-compatible with CUADB_DATABASE_URL. Avoid implementing a browser-submitted arbitrary URL form in this task.',
        checks: [
          'Define a validated server-side profile configuration format with stable identifiers.',
          'Maintain compatibility with the single PostgreSQL URL default.',
          'Bound total active connection pools and close unused resources safely.',
          'Expose safe profile labels, engine types and connection state to templates.',
          'Test duplicate names, invalid URLs, profile removal and secret redaction.',
        ],
        dependsOn: ['contracts-state', 'mysql-connect', 'sqlite-connect'],
      },
      {
        key: 'profiles-switch',
        title: 'Build secure connection switching and database selection UX',
        goal: 'Offer an understandable connection picker, accurately update the schema tree and selected relation, and avoid retaining stale rows from a previously selected database. Keep navigation responsive without storing secrets on the client.',
        context:
          'Switching connections changes the database trust boundary, catalog and permissions. HTMX requests must be tied to a stable configured profile ID and not accept arbitrary database hosts. Browser navigation should not display records from another profile after rapid switching or a failed connection.',
        checks: [
          'Render available trusted profile labels with clear connected and offline states.',
          'Reset or scope table selection, sorting and cursors when a profile changes.',
          'Reject unknown or removed profile identifiers without exposing connection details.',
          'Test rapid switching and inaccessible databases for stale response leakage.',
          'Document the connection selection behavior in the README.',
        ],
        dependsOn: ['profiles-config', 'explorer-states'],
      },
      {
        key: 'profiles-preferences',
        title: 'Polish non-sensitive preferences, navigation and keyboard shortcuts',
        goal: 'Remember convenient presentation preferences such as theme, page size and sidebar width without persisting database rows, raw SQL or credentials in the browser. Make common explorer actions easy to discover and use.',
        context:
          'This task is about UX state, not connection or record storage. Any client-side settings should be strictly allowlisted and small. Global shortcuts should not interfere with browser or text-input behavior, while focus management must remain correct after HTMX swaps.',
        checks: [
          'Persist theme and selected grid presentation settings without saving database values.',
          'Add discoverable shortcuts for search, refresh and navigation where accessible.',
          'Ensure preference defaults work when local browser storage is unavailable.',
          'Test across page reloads, HTMX swaps and keyboard-only navigation.',
          'Verify that no secrets, row snapshots or connection URLs enter persisted settings.',
        ],
        dependsOn: ['profiles-switch', 'grid-keyboard'],
      },
    ],
  },
  {
    key: 'release',
    title: '12 — Security, integration, performance and Docker release',
    goal: 'Prove CuaDB is safe and dependable as a local-first read-only database viewer for the supported engines. Ensure reproducible builds, meaningful automated tests, bounded query and memory behavior, documentation for real connections and a single production Docker container with one published UI port.',
    approach:
      'Create disposable database fixtures and automated adapter contract tests, then exercise HTTP/HTML navigation through smoke and end-to-end checks. Review attacker-controlled inputs, secrets and read-only guarantees. Verify multi-stage Docker builds, runtime resource limits and static assets. Benchmark representative large datasets to find pagination and rendering bottlenecks; publish clear compatibility and operational documentation.',
    risks:
      'A passing unit test does not prove the container can reach PostgreSQL or that database privileges prevent writes. Releasing with outdated lockfiles, missing build assets, expensive default SQL or unsafe Docker port binding would defeat the project goals. Use explicit acceptance gates and distinguish native-only checks from optional integration tests requiring disposable databases.',
    tasks: [
      {
        key: 'release-security',
        title: 'Complete a threat-model review and negative security test suite',
        goal: 'Verify the browser cannot cause writes, inject SQL identifiers, render malicious HTML, access unintended local files or reveal secrets through logs and history. Make each security control observable through a repeatable negative test.',
        context:
          'CuaDB opens databases that may hold sensitive data, even though its goal is only browsing. Threats include user-controlled query parameters, malformed database values, unsafe browser caching, SQLite path traversal and connection URL disclosure. A read-only database role remains the most important final defense.',
        checks: [
          'Document entry points, trust boundaries, sensitive assets and threat assumptions.',
          'Test identifier injection, XSS strings, enormous values and invalid cursors.',
          'Verify read-only credentials and timeout behavior for supported engines.',
          'Inspect browser caching, response headers, logs and clipboard behavior for leaks.',
          'Record residual risks and required controls before exposing the port beyond localhost.',
        ],
        dependsOn: ['http-security', 'sqlite-info', 'mysql-stats'],
      },
      {
        key: 'release-docker',
        title: 'Harden and verify the single-container production image',
        goal: 'Ship a minimal runtime image containing only the Rust executable and bundled static assets, with one host-published web port and access to external database servers through Docker networking. Docker must not bundle PostgreSQL or a second web service.',
        context:
          'The existing Dockerfile already has separate Bun and Rust build stages. Production deployment should lock dependency versions, run as a non-root user, avoid writable app directories and provide reasonable resource limits. The server should bind internally for Docker forwarding while the default host published port remains loopback-only.',
        checks: [
          'Verify reproducible Docker builds with committed Cargo and Bun lockfiles.',
          'Run image as non-root with minimal writable filesystem and dropped privileges where practical.',
          'Publish only the web port on 127.0.0.1 by default and document remote access risks.',
          'Test connection from CuaDB to postgis on a shared user-defined Docker network.',
          'Verify clean startup, shutdown, health and bundled CSS/HTMX behavior in Docker.',
        ],
        dependsOn: ['foundation-quality', 'profiles-switch'],
      },
      {
        key: 'release-integration',
        title: 'Establish disposable database fixtures and end-to-end acceptance tests',
        goal: 'Verify that the actual browser routes, adapter queries and templates work together for PostgreSQL, MySQL, MariaDB and SQLite. Tests must not modify an existing user database and should be explicit about which fixtures are required.',
        context:
          'A unit-level fake adapter proves HTTP behavior but cannot establish real catalog compatibility. Use disposable instances, seeded schemas and a least-privilege viewer role. End-to-end checks should cover schema browsing, first page, next page, sorting, structure, info and a connection failure path for each supported engine.',
        checks: [
          'Add isolated PostgreSQL/PostGIS, MySQL/MariaDB and SQLite integration fixtures.',
          'Run query and metadata acceptance tests with a restricted read-only user.',
          'Exercise real HTMX routes and verify sanitized HTML output and paging state.',
          'Keep ordinary unit tests runnable offline without Docker or databases.',
          'Document fixture cleanup, local commands and any optional test dependencies.',
        ],
        dependsOn: ['catalog-compat', 'grid-keyboard', 'mysql-stats', 'sqlite-info'],
      },
      {
        key: 'release-performance',
        title: 'Measure large-table navigation, rendering and memory behavior',
        goal: 'Keep the viewer responsive on databases with many schemas, thousands of tables and large relations. Benchmark actual query plans and response sizes before introducing cache or concurrency complexity.',
        context:
          'Rust alone does not make deep offset queries or unindexed sorts inexpensive. Typical bottlenecks are database round trips, large cells, HTML generation and excessive catalog enumeration. Metrics should separate database execution, rendering and total request time while avoiding record-content logging.',
        checks: [
          'Create representative large-table and wide-table fixtures with safe synthetic data.',
          'Measure first-page, deep-page, sort, metadata and repeated HTMX navigation timings.',
          'Verify configured query deadlines, memory caps and bounded response payload sizes.',
          'Record and address clear hotspots using query plans and targeted optimization.',
          'Document practical limits and when cursor pagination is available or unavailable.',
        ],
        dependsOn: ['release-integration', 'rows-pagination'],
      },
      {
        key: 'release-docs',
        title: 'Publish verified setup, supported engines and release acceptance guide',
        goal: 'Make CuaDB easy to clone, configure, test and operate on Fedora or another supported host. Documentation must distinguish implemented features from planned work and provide concrete examples for existing PostgreSQL-in-Docker, remote databases and SQLite mounts.',
        context:
          'The README began as an app-shell plan and includes commands validated by the user. As adapters become operational, replace planned-only language with verified support claims, tested environment variables and real connection troubleshooting. Users should understand Docker network setup, least-privilege role creation, limitations and expected error states.',
        checks: [
          'Update README with verified native and Docker workflows for each supported engine.',
          'Document connection URLs, database permissions, TLS and read-only mounts safely.',
          'Add a compatibility matrix and explicit known limitations for metadata and pagination.',
          'Include a release checklist that exercises check, test, build and container smoke flows.',
          'Record how Task groups were completed, validated and handed off without duplicating task records.',
        ],
        dependsOn: ['release-security', 'release-performance', 'release-docker'],
      },
    ],
  },
  {
    key: 'mssql',
    title: '13 — SQL Server adapter (post-core expansion)',
    goal: 'Evaluate and implement Microsoft SQL Server support after the PostgreSQL, MySQL/MariaDB and SQLite viewer is proven. Preserve the same read-only explorer, data grid and metadata contract while respecting SQL Server-specific drivers, namespaces, catalog views and pagination requirements.',
    approach:
      'First verify a well-maintained Rust SQL Server client and safe TLS/authentication options. Integrate driver-specific connection management and catalog discovery behind the shared adapter interface. Then implement bounded data pages, sorting, type handling and storage statistics where feasible. Test against a disposable supported server version and document differences transparently.',
    risks:
      'SQL Server driver maturity, TLS defaults and corporate authentication mechanisms may differ from SQLx-supported databases. Avoid claiming compatibility before end-to-end verification. Some index, storage and statistics operations may require privileges beyond simple SELECT; unsupported metadata must remain visibly unavailable rather than cause application failure.',
    tasks: [
      {
        key: 'mssql-spike',
        title: 'Evaluate SQL Server Rust driver, TLS and connection contract',
        goal: 'Validate the feasibility and security of a SQL Server adapter using an actively maintained Rust driver before committing to the implementation. Produce a clear go/no-go decision and compatible URL, timeout and authentication strategy.',
        context:
          'SQL Server is a common database engine but is not covered by the initial SQLx driver choice. External dependencies and TLS handling need additional review. The adapter should be optional and isolated so a failure to support a particular authentication mode cannot destabilize CuaDB’s core engines.',
        checks: [
          'Evaluate supported Rust drivers, maintenance status, TLS and licensing constraints.',
          'Prototype a read-only connection against a disposable SQL Server instance.',
          'Specify supported URL format, credentials and clear unsupported auth modes.',
          'Record capability differences and any required package/runtime changes.',
          'Document a go/no-go decision before building user-facing SQL Server routes.',
        ],
        dependsOn: ['release-integration'],
      },
      {
        key: 'mssql-catalog',
        title: 'Implement SQL Server schema, relation and column discovery',
        goal: 'Expose SQL Server databases and schemas, tables and views, columns, keys and indexes through CuaDB’s adapter interface. Do not force SQL Server catalog behavior into PostgreSQL-only models.',
        context:
          'SQL Server has distinct catalog tables, identifier quoting conventions and permissions for metadata visibility. The user should see supported objects without being granted administration rights. Missing permissions should appear as clear unavailable states rather than generic 500 errors.',
        checks: [
          'Discover accessible schemas, tables and views with correct identifiers.',
          'Return native types, nullable columns, keys and indexes where permitted.',
          'Use SQL Server-safe quoted identifiers and validated metadata lookups.',
          'Handle case-insensitive and mixed-case collations correctly in navigation.',
          'Verify against disposable fixtures under a restricted read-only login.',
        ],
        dependsOn: ['mssql-spike'],
      },
      {
        key: 'mssql-rows',
        title: 'Add SQL Server bounded rows, sorting and honest Info metadata',
        goal: 'Complete basic SQL Server browsing with paginated data, safe column sorting and available table-level metadata, using the same grid and inspector templates already supported by other engines.',
        context:
          'SQL Server paging uses different syntax and can require ORDER BY. Type conversions, large values and available storage statistics may differ from PostgreSQL. The adapter must define reliable sort behavior, apply query deadlines and expose metrics only when they can be retrieved with the configured privileges.',
        checks: [
          'Fetch bounded pages and enforce relation/column allowlists for sorting.',
          'Normalize common scalar and large-value types into the shared cell model.',
          'Report supported constraints and Info statistics with clear availability flags.',
          'Exercise real end-to-end browser flows using SQL Server fixture data.',
          'Document supported driver/server versions and unresolved engine limitations.',
        ],
        dependsOn: ['mssql-catalog'],
      },
    ],
  },
];
