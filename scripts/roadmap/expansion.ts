import type { GroupSpec } from './types';

export const sqliteGroups: GroupSpec[] = [
  {
    key: 'sqlite',
    title: '10 — SQLite file-based database adapter',
    goal: 'Complete the second required backend adapter after PostgreSQL and before frontend implementation. Browse local SQLite database files securely through the common backend contract. A database file must be mounted into the container or available through a configured host path; ordinary browsing remains read-only.',
    approach:
      'Treat SQLite as the required second backend engine, not a later multi-engine expansion. Finish its connection, catalog, bounded rows, typed values and capability reporting before starting the frontend. Define allowed file path roots and open files read-only, using sqlite_schema and PRAGMA metadata. Reuse common row/pagination models and document SQLite dynamic typing and unavailable statistics.',
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
        dependsOn: ['sqlite-rows'],
      },
    ],
  },
];

// MySQL/MariaDB and release work follow the PostgreSQL + SQLite backend and complete frontend.
export const expansionGroups: GroupSpec[] = [
  {
    key: 'mysql',
    title: '09 — MySQL and MariaDB compatibility',
    goal: 'Finish only the remaining MySQL/MariaDB compatibility work after the PostgreSQL + SQLite backend and complete frontend milestones are accepted. Preserve any already-completed connection groundwork; do not expand or verify these engines during the core milestone.',
    approach:
      'Resume this group only after frontend groups 06–08 are accepted. Inspect existing completed MySQL/MariaDB work first, then implement only missing catalog, paging and metadata behavior needed for the common UI. Use a small number of disposable fixtures for focused acceptance; do not build a large cross-version matrix or repeat PostgreSQL/SQLite verification.',
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
        dependsOn: ['mysql-connect', 'grid-keyboard', 'inspect-postgis'],
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
    key: 'auth',
    title: '13 — Authentication and resource permissions',
    goal: 'Add a secure local administrator bootstrap, user accounts, sessions and deny-by-default permissions before CuaDB exposes protected multi-user or write-capable features. Authorization must be scoped to trusted connection profiles and database resources.',
    approach:
      'Start with an explicit one-time administrator bootstrap that never ships a default password. Add strong password hashing, secure server-side sessions, CSRF protection and rate limits. Define roles and resource grants for connection profiles, schemas, relations and actions. Audit security-relevant events without recording credentials or row contents.',
    risks:
      'Authentication can create a false sense of safety if routes, HTMX fragments, exports or profile switching bypass the same authorization layer. Keep authorization in the server-side request path, deny by default, protect bootstrap secrets and avoid logging sensitive data. Do not expose the server beyond loopback until the security gate is accepted.',
    tasks: [
      {
        key: 'auth-bootstrap',
        title: 'Establish secure first-run administrator bootstrap',
        goal: 'Provide an explicit and safe first-run process for creating the initial CuaDB administrator. Do not ship default credentials, expose an unauthenticated registration endpoint or store bootstrap secrets in the browser. The operator must be able to complete setup locally and then disable the bootstrap path.',
        context:
          'CuaDB is currently local-first and does not yet have application users. The first administrator is the root of trust for later profile access and SQL permissions. Bootstrap should use a one-time operator-supplied secret or an interactive local setup command, expire after successful use and fail closed if setup state is ambiguous.',
        checks: [
          'Implement a one-time administrator bootstrap with no built-in username/password pair.',
          'Keep bootstrap secrets out of logs, URLs, HTML, browser storage and error messages.',
          'Disable or expire bootstrap after the first administrator is created.',
          'Restrict initial setup to the local operator and document recovery behavior.',
          'Test repeated setup, invalid secrets and interrupted bootstrap safely.',
        ],
        dependsOn: ['profiles-switch', 'foundation-quality'],
      },
      {
        key: 'auth-users',
        title: 'Implement secure user accounts and password storage',
        goal: 'Create the minimal user model and account lifecycle required for multiple people to use CuaDB safely. Store only strong password hashes, normalize unique login identifiers and make account state explicit. Keep account management separate from database credentials and connection profiles.',
        context:
          'Application users are not database users. CuaDB must never reuse a database password as an application password or expose database credentials to a logged-in user. Use a well-maintained password hashing implementation with per-password salts and parameters suitable for interactive logins.',
        checks: [
          'Define user identity, status, role assignment and unique login identifier models.',
          'Hash passwords with a modern memory-hard password hashing algorithm.',
          'Never store or return plaintext passwords or database credentials.',
          'Add safe create, disable and password-change flows for authorized administrators.',
          'Test duplicate identifiers, disabled users and password verification failures.',
        ],
        dependsOn: ['auth-bootstrap'],
      },
      {
        key: 'auth-session',
        title: 'Add secure login, logout and server-side session handling',
        goal: 'Implement authenticated browser sessions with predictable expiry, rotation and logout behavior. Protect state-changing requests against CSRF and prevent session fixation, cookie leakage and brute-force login attempts. Keep the existing local disconnected shell usable when authentication is not yet configured.',
        context:
          'The application will eventually support profile switching and controlled SQL operations. Authentication must be enforced consistently for full-page routes, HTMX fragments, downloads and future APIs. Prefer opaque server-side sessions over long-lived browser-readable tokens.',
        checks: [
          'Implement login and logout with opaque server-side session identifiers.',
          'Set secure, HttpOnly, SameSite cookies with configurable expiry and rotation.',
          'Add CSRF protection to state-changing requests and rate-limit failed logins.',
          'Ensure protected routes and downloads reject missing, expired or revoked sessions.',
          'Test session fixation, logout, expiry, cookie flags and CSRF failures.',
        ],
        dependsOn: ['auth-users'],
      },
      {
        key: 'auth-rbac',
        title: 'Enforce role and resource permissions across profiles and data',
        goal: 'Add deny-by-default authorization for administrators, read-only users and explicitly permitted editors. Permissions must scope users to trusted connection profiles, schemas, relations and allowed actions so one user cannot browse or modify another user’s resources.',
        context:
          'A single global role is insufficient once CuaDB supports multiple connections and controlled data editing. Resolve permissions on the server for every route and operation, including catalog navigation, rows, exports, profile switching and SQL execution. Database grants remain an independent security boundary.',
        checks: [
          'Define roles and explicit view, export, edit and SQL-execution permissions.',
          'Scope grants to connection profiles and optionally schemas or relations.',
          'Enforce authorization in every relevant HTTP and HTMX handler, not only the UI.',
          'Default to deny and prevent privilege escalation through profile or identifier tampering.',
          'Add positive and negative tests for cross-profile and per-resource access.',
        ],
        dependsOn: ['auth-session'],
      },
      {
        key: 'auth-audit',
        title: 'Record security events and administrator access changes',
        goal: 'Provide a minimal audit trail for security-sensitive actions so operators can understand who accessed CuaDB and who changed permissions. Record login outcomes, session revocation, profile access and permission changes without storing passwords, connection URLs, SQL secrets or database row contents.',
        context:
          'Auditability is required before enabling controlled write operations. Keep events append-only from application code, bound retention and avoid turning audit logs into a second copy of database contents. Administrative actions must be attributable to an authenticated user.',
        checks: [
          'Record authentication outcomes, session revocations and permission changes.',
          'Record profile access and later SQL/edit actions with user and resource identifiers.',
          'Redact credentials, raw connection strings, query parameters and row contents.',
          'Restrict audit viewing to authorized administrators and bound retention/queries.',
          'Test audit coverage for denied access and successful privileged actions.',
        ],
        dependsOn: ['auth-rbac'],
      },
    ],
  },
  {
    key: 'sql-workspace',
    title: '14 — SQL workspace and controlled database editing',
    goal: 'Add a separate, explicitly authorized workspace for SQL execution and normal table data editing. Preserve the safe read-only explorer as the default experience. Every mutating action must be authenticated, permission-checked, bounded, attributable and auditable.',
    approach:
      'Implement a server-side SQL policy and capability boundary before building the editor. Use a real SQL parser for statement classification rather than regular expressions, enforce per-profile permissions and query limits, and keep result rendering bounded. Add explicit CRUD operations and transaction controls as separate typed actions. Never accept credentials or arbitrary connection targets from the browser.',
    risks:
      'A SQL workspace can turn a local viewer into a destructive database client. Parser bypasses, multi-statements, dangerous functions, privilege escalation, unbounded results and accidental commits are key risks. Default the feature off, require explicit operator enablement and per-user grants, rely on least-privilege database accounts and audit every mutation.',
    tasks: [
      {
        key: 'sql-policy',
        title: 'Define SQL statement policy and authorized execution boundary',
        goal: 'Create a server-side execution boundary that classifies submitted SQL and checks the authenticated user’s permissions for the selected trusted profile. The default must remain disabled. Reject unsupported or dangerous statement classes, multi-statement payloads and resource-exhausting requests before contacting the database.',
        context:
          'The current explorer deliberately has no arbitrary SQL endpoint. This task establishes policy before any editor UI or write operation exists. Use a maintained SQL parser and engine capability flags; never treat regex checks as a security boundary. Database grants and CuaDB authorization must both permit the requested operation.',
        checks: [
          'Add parser-backed statement classification for supported PostgreSQL, SQLite and MySQL-family dialects.',
          'Default SQL execution to disabled and require explicit operator configuration plus user permission.',
          'Reject multi-statements and disallowed administrative, file, network and privilege-changing operations.',
          'Apply statement deadlines, result row/byte caps, cancellation and redacted errors.',
          'Test parser edge cases, denied statements and per-profile permission failures.',
        ],
        dependsOn: ['auth-rbac', 'mysql-stats', 'sqlite-info'],
      },
      {
        key: 'sql-workspace-ui',
        title: 'Build the authenticated SQL workspace and bounded result viewer',
        goal: 'Provide a separate SQL workspace for authorized users with a clear editor, execution controls, status feedback and a bounded result grid. Keep the existing database explorer intact and make it obvious which connection profile is selected and which permissions apply.',
        context:
          'This is a later frontend feature, not part of the initial canonical explorer milestone. Reuse the approved visual system and keep SQL execution server-side. Do not store credentials or sensitive result sets in browser history, local storage or long-lived client caches.',
        checks: [
          'Add a distinct SQL workspace entry visible only to users with the relevant permission.',
          'Show the selected trusted profile and effective read/write permission before execution.',
          'Render bounded typed results, affected-row counts and safe execution errors.',
          'Support cancellation and prevent stale results from replacing a newer profile selection.',
          'Verify keyboard accessibility and that unauthorized users cannot invoke the routes directly.',
        ],
        dependsOn: ['sql-policy', 'grid-keyboard'],
      },
      {
        key: 'sql-crud',
        title: 'Implement permission-checked table create, update and delete operations',
        goal: 'Add normal table data editing through explicit typed operations rather than silently making the existing explorer grid editable. Validate relation and column identities against catalog metadata, bind values, enforce per-resource edit permissions and keep every operation within a transaction.',
        context:
          'CRUD is distinct from raw SQL execution and should remain available only for tables and columns the adapter can safely identify. Respect generated columns, identity values, constraints, foreign keys and database errors. Do not provide bulk destructive actions without explicit bounded confirmation.',
        checks: [
          'Implement create, update and delete operations using validated catalog identifiers and bound values.',
          'Enforce per-user, per-profile and per-relation edit permissions on the server.',
          'Respect generated/identity columns, constraints and adapter capability limitations.',
          'Require explicit confirmation and affected-row bounds for destructive operations.',
          'Test constraint failures, stale row versions and unauthorized mutation attempts.',
        ],
        dependsOn: ['sql-policy', 'profiles-switch'],
      },
      {
        key: 'sql-transactions',
        title: 'Add explicit transaction review, commit and rollback controls',
        goal: 'Make write behavior understandable and recoverable by exposing explicit transaction boundaries for supported editing workflows. Users must see the target profile, pending changes and affected-row estimates before committing, with a clear rollback path for uncommitted work.',
        context:
          'Database changes are irreversible once committed unless the database itself provides recovery. Avoid hidden auto-commit behavior for multi-step edits and never hold transactions open indefinitely. Respect each engine’s transaction semantics and surface unsupported capabilities honestly.',
        checks: [
          'Provide explicit transaction begin, commit and rollback handling for supported edit flows.',
          'Show the target profile, operation summary and affected-row count before commit.',
          'Apply transaction deadlines and roll back on cancellation, disconnect or validation failure.',
          'Prevent profile switching while a transaction is pending unless it is safely rolled back.',
          'Test rollback, commit failures, timeouts and concurrent row changes.',
        ],
        dependsOn: ['sql-crud'],
      },
      {
        key: 'sql-audit',
        title: 'Audit SQL execution and database mutations',
        goal: 'Make every SQL execution and CRUD mutation attributable and reviewable. Store the authenticated actor, profile, operation category, timestamp, outcome and bounded metadata without copying sensitive row values or credentials into the audit log.',
        context:
          'The SQL workspace is a privileged feature. Audit events complement database-native logs and must not expose secrets or create an unbounded second data store. Failed and denied attempts are as important as successful writes.',
        checks: [
          'Record authorized and denied SQL execution attempts with actor, profile and outcome.',
          'Record CRUD and transaction commit/rollback outcomes without storing row contents.',
          'Redact secrets and bound query text or store only a safe normalized statement summary.',
          'Restrict audit access and document retention and export behavior.',
          'Test audit events for successful, failed, denied and rolled-back operations.',
        ],
        dependsOn: ['sql-transactions', 'auth-audit'],
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
        dependsOn: ['http-security', 'sqlite-info', 'mysql-stats', 'auth-audit', 'sql-audit'],
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
        dependsOn: ['foundation-quality', 'profiles-switch', 'auth-session'],
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
        dependsOn: [
          'catalog-compat',
          'grid-keyboard',
          'mysql-stats',
          'sqlite-info',
          'auth-rbac',
          'sql-audit',
        ],
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
];
