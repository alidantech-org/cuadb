---
{id: DOC-D56A5E2F9D66,name: Server-rendered UI and HTMX interaction contract,type: documentation,tags: [cuadb,architecture]}
---

# Server-rendered UI and HTMX interaction contract

## Frontend architecture
Askama renders a single explorer page and reusable HTML fragments. Tailwind provides layout, typography, consistent tokens and responsive states. A small custom grid CSS layer handles sticky headers, horizontal scrolling, high-density rows and resize behavior. HTMX handles browser-to-Rust navigation and partial updates; vanilla JavaScript handles immediate local actions such as sidebar toggling, resizing, copying and keyboard shortcuts. No React, Next.js, JSON-first state store or frontend build server is required.

## Navigation and data contracts
The sidebar shows trusted connection labels and, within each database, supported schemas or namespaces and relation types. Expanding a schema lazily requests a fragment. Selecting a table loads its first data page, an explicit Structure tab and an Info tab. Every server route validates relation identity, sort, requested columns and bounded page controls before issuing database requests. The URL may preserve non-sensitive navigation context but never connection credentials, cell values or raw SQL.

## Fragment behavior
Each fragment owns a stable target: connection state, schema subtree, data grid, or inspector content. Fast repeated clicks should cancel or serialize conflicting requests so older results never overwrite the current selection. Every fragment needs an empty, loading, permission-denied, unsupported-capability and timeout state. Server-rendered messages should be accessible and accurately describe what happened. The same view models must support a full-page reload without relying on previously cached JavaScript state.

## Grid rules
The records grid is semantic HTML and remains read-only. It shows actual column names, typed values, visible NULL markers, sortable headers and previous/next navigation using real continuation state. Display only a bounded page, usually 50 rows, never an entire table. Large JSON, arrays, bytea and spatial values are truncated with a safe detail view; copying a cell is a browser-only action. The UI must distinguish estimated row counts from exact counts and disclose when a table has no stable pagination key.

## Accessibility and verification
Use visible keyboard focus, aria-sort on active sortable headers, labeled buttons and appropriately announced loading or failure feedback. Test narrow viewports, long column names, Unicode names, many columns, dark/light themes, rapid sorting, stale HTMX responses and large-cell detail interactions. Keep event handlers delegated to persistent elements or explicitly reinitialized after swaps. No browser-only interaction should start a database query unnecessarily.