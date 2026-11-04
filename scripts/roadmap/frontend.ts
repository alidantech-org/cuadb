import type { GroupSpec } from './types';

// Browser-facing behavior is intentionally server rendered; JavaScript remains small.
export const frontendGroups: GroupSpec[] = [
  {
    key: 'http',
    title: '05 — HTTP read endpoints and HTML fragments',
    goal: 'Expose the database adapter through safe Axum routes that return full pages and HTMX fragments. Keep typed parsing, permission handling and server state separate from Askama templates. Browser requests may select an existing connection, relation, page and sort, but may not supply arbitrary SQL or raw external connection targets.',
    approach: 'Define an explicit routing contract for home, connection status, schema children, table data, structure and statistics. Parse identifiers and pagination controls through typed request models and translate adapter errors into safe HTTP statuses. Render full navigation pages and narrow fragments from the same view models. Add request cancellation, caching boundaries and secure defaults before wiring the polished UI.',
    risks: 'An HTML-fragment application can accidentally render private data into browser history or shared caches. Cross-origin requests, raw diagnostic strings and unbounded HTMX updates are other risks. Disable caching for sensitive data, escape values and make browser state transitions predictable. The server must not turn query parameters into unvalidated SQL identifiers.',
    tasks: [
      {
        key: 'http-routes',
        title: 'Implement typed read-only Axum route contracts for browsing',
        goal: 'Design and implement HTTP GET endpoints for connection status, schemas, tables, data rows, structure and info. Routes should delegate to adapters, return stable status codes and support both direct navigation and HTMX requests.',
        context: 'The existing router serves a shell and a status fragment. Future handlers must not issue inline vendor-specific SQL or pass raw database driver objects into templates. Each route needs a validated relation identifier and clear output semantics, including what to do when a table becomes inaccessible between requests.',
        checks: [
          'Define documented route paths and typed path/query request parsing.',
          'Delegate discovery and row reads to the adapter through application state.',
          'Reject invalid identifiers, directions, page sizes and unsupported relation types.',
          'Support full-page requests and HTMX fragment responses where appropriate.',
          'Add isolated router tests with a fake adapter rather than requiring a live database.'
        ],
        dependsOn: ['contracts-types','foundation-errors']
      },
      {
        key: 'http-views',
        title: 'Build Askama view models and reusable fragment templates',
        goal: 'Create compile-time-checked templates and data-focused view models for connection state, table navigation, rows and metadata. Keep display logic outside SQL implementation and ensure all database-provided strings are safely escaped.',
        context: 'Askama templates are currently limited to a demo page. The real explorer should reuse consistent rendering logic for a full-page load and an HTMX partial update. Empty, restricted and offline states must be first-class view model variants rather than a tangle of template conditionals.',
        checks: [
          'Create typed view models for sidebar tree, relation header, grid and inspector tabs.',
          'Reuse components across full pages and HTMX responses without duplicate markup.',
          'Verify HTML escaping for suspicious table names, cell contents and error messages.',
          'Render explicit empty and inaccessible states with clear next actions.',
          'Keep template code free of direct driver calls or raw SQL statements.'
        ],
        dependsOn: ['http-routes']
      },
      {
        key: 'http-security',
        title: 'Harden browser responses, headers and fragment cache behavior',
        goal: 'Protect local database content from accidental exposure through HTTP caches, browser history snapshots and cross-origin interactions. Keep web access on localhost by default and document the boundary for any later remote deployment.',
        context: 'CuaDB is a browser-based interface to sensitive database values. Even read-only responses may contain personal or proprietary information. HTMX must not persist data snapshots unexpectedly, while CSS and JS should be bundled locally. Security headers should permit intended HTMX behavior without opening permissive script or embedding policies.',
        checks: [
          'Add no-store cache behavior to all authenticated or database-bearing HTML responses.',
          'Define CSP, clickjacking and MIME-sniffing protections compatible with bundled assets.',
          'Disable HTMX history caching of sensitive database fragments and test navigation behavior.',
          'Verify that unsafe origins or methods cannot trigger unauthorized future state changes.',
          'Add response security regression tests and document local-only deployment assumptions.'
        ],
        dependsOn: ['http-views']
      },
      {
        key: 'http-errors',
        title: 'Design HTMX loading, cancellation, empty and error contracts',
        goal: 'Make asynchronous data navigation predictable during slow queries, network interruptions and permission failures. Stale responses should not overwrite the currently selected table, and errors should render in a consistent, accessible way.',
        context: 'Switching between tables, sorting and clicking next rapidly may produce overlapping requests. The UI must avoid showing mixed results for different relations. Distinguish empty dataset, connection offline, timeout, denied permission and invalid request. Browser-only interactions should remain available if the database is temporarily unreachable.',
        checks: [
          'Define swap targets, request synchronization and stable loading indicators per fragment.',
          'Provide safe fragments for timeout, validation, permission and disconnected states.',
          'Ensure stale responses cannot replace a newer selected relation or sort.',
          'Test full-page and HTMX failure behavior with simulated slow fake adapters.',
          'Document retry and refresh actions without automatic unbounded polling.'
        ],
        dependsOn: ['http-security']
      }
    ]
  },
  {
    key: 'explorer-ui',
    title: '06 — Database navigation and workspace UI',
    goal: 'Replace the demo shell with a clean, responsive database explorer focused on fast navigation. The left sidebar should display connections, schemas and relation names; the main panel should immediately show data or a meaningful empty state. Preserve the lightweight Rust-rendered design without React, Next.js or a frontend state library.',
    approach: 'Use Askama components and Tailwind CSS tokens to create a coherent visual system, then connect sidebar actions using HTMX. Keep the most common navigation one click away, support filtering long table lists, and make loading/disabled states obvious. Use vanilla JavaScript only for local toggles such as resizing and keyboard handling, with no frontend data fetching duplicated in JavaScript.',
    risks: 'A crowded sidebar or expensive full-tree render makes database browsing unpleasant when a schema has thousands of tables. The UI must remain usable on small laptop screens, maintain accessible focus and not infer connection success from a stale indicator. Avoid rendering fake metadata when a relation has not yet loaded.',
    tasks: [
      {
        key: 'explorer-layout',
        title: 'Establish compact accessible layout, tokens and responsive panels',
        goal: 'Create a cohesive app frame with navigation, contextual toolbar, connection status and primary content area. Optimize for desktop and laptop widths, while preserving a useful fallback on narrow screens and accessible keyboard focus.',
        context: 'The starter uses utility classes directly inside one page. As the viewer grows, common colors, spacing, hover/focus states and dense table styles should be centralized. Tailwind builds must include all Askama templates, and specialized CSS can handle grid overflow and resizable panels without complex JS libraries.',
        checks: [
          'Refactor repeated presentation into reusable Askama components and theme tokens.',
          'Create accessible sidebar, toolbar, content area and empty/disconnected layouts.',
          'Ensure responsive behavior does not hide essential table navigation controls.',
          'Add visible focus and dark/light theme primitives with adequate contrast.',
          'Verify final build uses only bundled CSS and JS assets.'
        ],
        dependsOn: ['http-views']
      },
      {
        key: 'explorer-tree',
        title: 'Connect schema and table tree to real catalog data',
        goal: 'Show accessible databases or configured connections, schemas and supported relation types in a collapsible tree. Clicking a table should select it, update the main panel and preserve clear visual focus without a full-page reload.',
        context: 'The PostgreSQL adapter supplies typed relation metadata, while the sidebar must not expose inaccessible objects or raw catalog rows. Schemas can be expanded on demand, and common types need distinct labels. Name filtering should be quick and should not force loading every record from every relation.',
        checks: [
          'Load available schema nodes and relation lists through real adapter-backed routes.',
          'Support expand/collapse and distinguish tables, views and partition parents.',
          'Implement empty schema and permission-denied states without broken tree nodes.',
          'Preserve selected relation and active tab across HTMX navigation.',
          'Test tree behavior with many relations and unusual names.'
        ],
        dependsOn: ['explorer-layout','catalog-navigation','http-errors']
      },
      {
        key: 'explorer-search',
        title: 'Add fast relation-name search and navigation breadcrumbs',
        goal: 'Let users locate a table or view quickly even in databases containing hundreds of relations. Provide contextual schema and relation breadcrumbs so users understand the current selection without relying on the sidebar alone.',
        context: 'CuaDB is not a SQL search engine. The initial search filters relation names and schemas only; it must not query table contents or full-scan the database. Large trees may need server-side filtering or lazy loading, while small lists can be filtered locally with minimal JavaScript.',
        checks: [
          'Search schema and relation names without reading underlying data rows.',
          'Preserve navigation context and selection when clearing or changing the search.',
          'Provide keyboard navigation and clear empty-results messaging.',
          'Test special characters, case sensitivity and large table lists.',
          'Keep search requests bounded and debounce remote requests when used.'
        ],
        dependsOn: ['explorer-tree']
      },
      {
        key: 'explorer-states',
        title: 'Provide coherent disconnected, loading and error navigation states',
        goal: 'Make the interface trustworthy at every stage of database initialization. A user must be able to tell whether CuaDB itself is running, whether a database is connected and whether a selected table is loading or inaccessible.',
        context: 'The shell currently marks itself as not connected. The real interface should preserve that honesty when URLs are missing, connections are refused or the current user lacks permission. Avoid flickering status labels, stale schema trees and unlimited automatic retries.',
        checks: [
          'Render accurate connection indicators distinct from server health.',
          'Expose useful recovery guidance for offline or misconfigured database connections.',
          'Show accessible skeleton/loading and empty-content states without fake data.',
          'Prevent stale navigation fragments after fast table switching.',
          'Test accessibility announcements and focus retention during HTMX swaps.'
        ],
        dependsOn: ['explorer-search','contracts-state']
      }
    ]
  },
  {
    key: 'data-grid-ui',
    title: '07 — Read-only records grid and interactions',
    goal: 'Deliver the central CuaDB experience: an efficient, readable grid for inspecting records in permitted tables or views. Users should open a table and immediately see bounded data, click column headers to sort, move through pages and inspect large cells without accidentally editing anything.',
    approach: 'Use semantic HTML tables with sticky headers, clear type indicators and horizontal scrolling. Let Rust retrieve each page and Askama render complete grid fragments; HTMX swaps only the grid container. Keep browser-side JS narrow: resizing columns, copy-to-clipboard and keyboard shortcuts. Render large strings or JSON in a bounded detail panel rather than expanding every cell by default.',
    risks: 'Very wide tables, large JSON documents and stale HTMX responses can make simple pages feel heavy. The grid should never fetch entire relations in client memory or expose edit controls. Accessibility matters: sorting directions must be announced, focus should remain usable and keyboard controls must not trap navigation.',
    tasks: [
      {
        key: 'grid-table',
        title: 'Render typed records in an efficient semantic HTML grid',
        goal: 'Turn adapter row pages into a clean table with stable column ordering, readable type formatting and clear NULL values. Keep the layout usable with dozens of columns and avoid loading unnecessary data on the client.',
        context: 'Database tables frequently have mixed scalar, JSON and binary columns. Large content should be abbreviated in a predictable way and accessible on demand. HTML output must always escape values, and the selected relation name must be shown explicitly so users know which data they are viewing.',
        checks: [
          'Render row headers, column labels, typed values and explicit NULL markers safely.',
          'Provide horizontal scrolling and sticky headers on narrow or wide tables.',
          'Show accessible empty-table and unsupported-type states without fake rows.',
          'Bound initial rendered cell content and preserve a path for expanded inspection.',
          'Verify XSS escaping and layout behavior with fixture records.'
        ],
        dependsOn: ['rows-cell-types','explorer-tree']
      },
      {
        key: 'grid-pagination',
        title: 'Wire paging controls and page size to the Rust server',
        goal: 'Let users move between pages and choose common bounded sizes such as 25, 50 or 100 rows. Paging must preserve the selected relation and sort while clearly indicating whether next and previous pages are available.',
        context: 'The backend chooses an offset or keyset strategy based on table indexes. The UI must not promise exact total pages when only approximate row statistics are available. HTMX swaps should avoid resetting unrelated sidebar state or accidentally rendering data from an older request.',
        checks: [
          'Render previous/next buttons using actual backend continuation state.',
          'Offer bounded page-size choices without allowing arbitrary large values.',
          'Preserve relation, sorting and pagination cursor during HTMX navigation.',
          'Handle first, last and empty pages with accurate disabled states.',
          'Test rapid paging and stale request behavior against a fake adapter.'
        ],
        dependsOn: ['grid-table','rows-pagination']
      },
      {
        key: 'grid-sorting',
        title: 'Connect sortable headers with clear direction and stable state',
        goal: 'Allow one-column ascending or descending sorting with a clear indicator, keyboard support and bounded server-side queries. The UI must never sort just the current page locally while implying that the whole table is sorted.',
        context: 'Sorting is an explicit backend operation and may be slow for unindexed columns. The grid should announce the chosen direction, reset page position when needed and retain the table selection. Unsupported columns should not look interactive or trigger broken query paths.',
        checks: [
          'Generate sort links only for backend-approved column identifiers.',
          'Expose aria-sort and visible ascending/descending state on active headers.',
          'Reset or adapt paging state when sorting changes.',
          'Render a safe error and restore usable controls when a sort is rejected or timed out.',
          'Test sort transitions, NULL-heavy fixtures and fast repeated header clicks.'
        ],
        dependsOn: ['grid-pagination','rows-sort']
      },
      {
        key: 'grid-cell-detail',
        title: 'Add safe expanded cell detail and copy actions',
        goal: 'Let users inspect JSON, long text, arrays and other complex values without cluttering the grid. A detail drawer should show the complete supported value when within limits and clearly mark truncation or unavailable content.',
        context: 'Many database records include substantial JSON or serialized payloads. A naïve renderer would send megabytes per page. The detail request must remain read-only, identify the exact row and column safely and handle records with no stable row key honestly. Clipboard actions can use vanilla JavaScript without server calls.',
        checks: [
          'Build an accessible reusable detail panel with formatted text and JSON when supported.',
          'Use bounded detail fetches tied to validated relation, column and row identity.',
          'Show truncation and privacy-safe fallbacks for huge or unsupported values.',
          'Implement local copy actions without exposing credentials or unsafe HTML.',
          'Test large JSON, control characters, Unicode and missing-key row scenarios.'
        ],
        dependsOn: ['grid-table','rows-cell-types']
      },
      {
        key: 'grid-keyboard',
        title: 'Polish grid resizing, keyboard navigation and accessibility',
        goal: 'Make dense database browsing comfortable with the keyboard and usable for assistive technology. Keep client-side behavior lightweight and resilient across HTMX fragment replacement.',
        context: 'Resizable columns and shortcuts improve a developer tool, but custom browser behavior must not interfere with native table semantics. Delegate events from stable containers so swapped rows continue to work. Do not add a complete frontend framework or local data state store for these small interactions.',
        checks: [
          'Support focused table navigation, copy shortcuts and escape-to-close behavior.',
          'Add optional column width adjustments without breaking responsive overflow.',
          'Keep hover, focus, loading and error states readable in dark and light modes.',
          'Verify event handlers still work after multiple HTMX swaps.',
          'Test keyboard-only navigation and automated accessibility checks where feasible.'
        ],
        dependsOn: ['grid-sorting','grid-cell-detail']
      }
    ]
  },
  {
    key: 'inspectors',
    title: '08 — Structure, statistics and PostgreSQL rich values',
    goal: 'Complete the table inspection experience through Structure and Info panels and careful handling of PostgreSQL-specific value types. Users should inspect data definitions and storage without complex administration screens, and PostgreSQL/PostGIS objects should remain readable even when they do not map to common scalar types.',
    approach: 'Connect the catalog metadata to reusable table inspector templates. Show columns, defaults, keys and indexes under Structure, while Info holds estimated row counts and storage measurements. Add JSON, arrays and bytea formatting with bounded content. Detect PostGIS availability and offer metadata and safe GeoJSON representations, without adding a map dependency in the first release.',
    risks: 'Catalog statistics can be absent or stale and spatial objects can contain enormous coordinate arrays. Avoid automatic expensive geometry transformations and do not present estimated widths as exact storage usage. Keep errors understandable when a schema has custom types or when the connected role cannot inspect all metadata.',
    tasks: [
      {
        key: 'inspect-structure',
        title: 'Implement table Structure tab from real metadata',
        goal: 'Show ordered column definitions, nullability, defaults, identity flags, primary and foreign keys and relevant indexes in a readable layout. The tab must work for supported views as well as ordinary tables and explain when metadata does not apply.',
        context: 'Catalog information is supplied by the PostgreSQL adapter, not by direct queries inside Askama templates. Composite keys and cross-schema foreign keys need clear representation. Users should be able to understand a table at a glance without opening a secondary database management application.',
        checks: [
          'Render ordered columns and native data type names with correct key indicators.',
          'Expose indexes, constraints and foreign-key targets with safe relation links.',
          'Handle views, partition parents and inaccessible related objects gracefully.',
          'Test Structure tab with a complex fixture containing composite keys and defaults.',
          'Keep the tab responsive through HTMX fragment updates without page reloads.'
        ],
        dependsOn: ['catalog-relations','http-views']
      },
      {
        key: 'inspect-info',
        title: 'Implement Table Info tab with honest statistics and storage',
        goal: 'Display useful measurements including estimated rows, table data bytes, index bytes, combined size and relation type. Every value must be clearly identified as exact, estimated or unavailable where applicable.',
        context: 'PostgreSQL pg_stat and size functions expose useful metrics but have important semantics for partitioned and foreign tables. Expensive exact counts are excluded from default navigation. Cached values may be stale; the interface should offer a lightweight explicit refresh action.',
        checks: [
          'Render adapter-provided sizes, estimated rows and relation classification clearly.',
          'Explain estimates, unknown values and partition-related size caveats in the UI.',
          'Provide a bounded refresh action without forcing SELECT COUNT(*) over large tables.',
          'Test empty, partitioned, foreign and permission-restricted fixture scenarios.',
          'Use the same capability flags that other database adapters will expose.'
        ],
        dependsOn: ['catalog-stats','http-views']
      },
      {
        key: 'inspect-types',
        title: 'Improve PostgreSQL JSON, arrays, binary and extension value display',
        goal: 'Make complex PostgreSQL values readable through structured previews and safe detail formatting. Types such as jsonb, UUID, arrays, enum labels, timestamp with time zone and bytea should retain important semantics without bloating row pages.',
        context: 'The generic row model provides typed cells, but individual presentation strategies differ. JSON should be indented in a detail panel; binary data should show size or a limited encoding preview; timestamps should not silently lose zone information. Unknown extension types require a predictable fallback.',
        checks: [
          'Add bounded formatting for JSON/JSONB, arrays, enums and bytea.',
          'Preserve date/time zone, exact numeric and NULL semantics in displayed values.',
          'Prevent database strings from rendering as unsanitized HTML.',
          'Test deeply nested JSON, large arrays, invalid UTF-8 representations and binary values.',
          'Document type-specific rendering behavior and size limits.'
        ],
        dependsOn: ['rows-cell-types','grid-cell-detail']
      },
      {
        key: 'inspect-postgis',
        title: 'Add optional PostGIS geometry and geography inspection',
        goal: 'Detect supported PostGIS types and show geometry subtype, SRID and bounded spatial representations on demand. Keep spatial data optional, read-only and safe even when geometry columns contain large or complex objects.',
        context: 'The existing Fedora database image is postgis/postgis:18-3.6, but an extension is installed per database and may not exist in every connected database. PostGIS metadata can describe geometry/geography types; GeoJSON conversion should be explicit and bounded, avoiding automatic full-table transformations or a heavy mapping UI.',
        checks: [
          'Detect PostGIS extension availability and geometry/geography columns per database.',
          'Expose safe geometry type, SRID and optional bounded GeoJSON detail.',
          'Handle invalid, huge and non-4326 spatial values without crashes or misleading coordinates.',
          'Do not introduce map rendering or expensive bulk geometry conversion by default.',
          'Test on both PostGIS-enabled and plain PostgreSQL fixture databases.'
        ],
        dependsOn: ['inspect-types','catalog-compat']
      }
    ]
  }
];
