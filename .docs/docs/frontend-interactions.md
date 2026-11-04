---
{id: DOC-D56A5E2F9D66,name: Server-rendered UI and HTMX interaction contract,type: documentation,tags: [cuadb,architecture]}
---

# Server-rendered UI and HTMX interaction contract

## Binding visual and interaction reference

**Canonical design asset: `assets/cuadb-ui-demo.html` (repository root).** This is the required implementation specification for the CuaDB frontend, not an optional inspiration board. The eventual Rust/Askama/HTMX app must reproduce its visible composition, spatial proportions, controls, colors, typography, density, modes, inspector and settings with high fidelity. A developer must open the standalone HTML in a browser, exercise each state, and compare the real app beside it before declaring frontend tasks complete. Preserve the original demo as a reference unless a user explicitly authorizes changes. Reuse its design tokens and CSS semantics while decomposing the implementation into maintainable Askama templates, Tailwind/custom CSS and minimal browser JavaScript. Do not introduce React, an unrelated design system, a new global top navigation bar or large generic dashboard widgets.

The demo contains **simulated records, connection names, PostgreSQL statistics and client-side export behavior**. These are examples only. In the production interface use actual, permission-scoped database metadata and rows. Preserve the same interactions and visible states, but implement unbounded operations safely (cancellable, bounded or streamed on the server); do not silently fetch whole tables into browser memory or suggest that a fake connection is live. Security, data truthfulness and accessibility take precedence where behavior conflicts, but visual departures require an explicit written rationale and user review.

## Phase 2 decisions (2026-11-05, owner)

- **Baseline plus improvements.** The updated demo is the baseline for look, feel, density and interaction ideology. It may be improved where that makes the product better; the "no departures" wording above is superseded. Record each deliberate departure in a Task comment.
- **New surfaces from the updated demo:** login and sign-out, Account block in settings, roles and access tags (`none`, `read`, `write`, `destructive`), connection picker plus database picker, connection create/edit/test/delete modal, Administration view (Members, Connections, Permissions, Audit log, Security), typed row editor with insert and delete, and the SQL workspace with a pending-change bar.
- **Server truth over demo simulation.** Any-password login, client-side audit, client-side statement classification, the runtime deployment-mode toggle and the simulated connection test are demo conveniences only. In production the server authenticates, authorizes, classifies SQL with its parser and writes the append-only audit log. The Security tab shows deployment mode, bind address and session lifetime as read-only operator configuration.
- **Connection management is real.** Administrators create, edit, test and delete connection profiles through an admin-only backend; secrets are encrypted at rest, never returned to the browser, and a blank password on edit keeps the stored secret.
- **Content-Security-Policy.** Scripts and styles come only from `/static`. No inline `<script>`, `style=` attributes or `on*=` handlers; bar widths, swatches and similar values use classes, `data-*` attributes and CSS custom properties applied by bundled JavaScript.
- **CSRF.** The layout exposes the session CSRF token in a meta tag and the bundled script adds `x-csrf-token` to every HTMX request.
- **Verification.** Manual side-by-side comparison in the built-in browser at 1440x900, 1280x800 and 390x844, light and dark, plus Rust tests for behavior. No browser-test dependency is added.

## Canonical layout, tokens and responsive appearance

- Reference composition: full-height application without a separate global header; compact left sidebar with search at the top and connection/settings controls at the bottom; draggable 5px splitter; remaining width devoted to the selected object's main panel. Default sidebar width **264px**, adjustable between **180px and 520px**, reset by double-click. Mobile drawer and inspector overlay appear at **760px or narrower**.
- Reference visual tokens: `--bg`, `--sf`, `--sf2`, `--bd`, `--tx`, `--mu`, `--ac`, `--acs`, `--ok`, `--wr`, `--rad`, `--rh`, `--sw` and `--sh`. Preserve the neutral light/dark surfaces, teal default accent, subtle dividers, 12.5px default system font, 6px default radius, compact 26px buttons, 24px default grid rows and restrained hover/focus treatment.
- Sidebar: `Search tables…` field; schema folders; relation icons and selection state; expandable names and optional column children; displayed rows/size and icon styles; table-selection mode; bottom connection picker plus settings gear. Simple mode shows a compact schema/table tree. Advanced mode adds Overview and per-schema groups for Tables, Views, Functions, Procedures, Sequences, Types and Triggers, with relevant counts. Show only objects supported by the engine and visible to the connected role.
- Main table toolbar: compact schema-qualified relation heading, real row/size metadata, `Search rows…` field, Columns popup, Compact control, Export popover, and **Data / Structure / Indexes** segmented switcher. Do not replace the reference with an `Info` tab. Actual Indexes screen contains the index list and Storage section. Advanced Overview is separate, with small cards, size-by-schema bars, largest tables and extensions when available.
- Data grid: sticky column header and row-number rail; tiny type labels and primary-key markers; compact hairline dividers; horizontal overflow; aligned numerical cells; NULL and type-specific pill or monospace treatments; selected/hover states; right-side **310px** selected-row inspector with all column/value details and close affordance, converted to an overlay on narrow screens. Footer has previous/next controls, displayed range, visible-column count, **25 / 50 / 100** page-size selector (default **25** in the reference) and read-only `SELECT only` indicator.
- Theme and settings options must mirror the demo: Explorer Simple/Advanced; Theme Auto/Light/Dark, Mist/Slate/Sand/Mono palette, accent presets/custom color and Sharp/Soft/Round corners; Type text S/M/L, Sans/Serif/Mono and monospace data cells; Grid XS/S/M/L height, Rows/Grid/None lines, NULL/∅/Blank, row numbers, column type labels, stripes, wrapping and separators; Sidebar color/mono/off icons, row-count/size label and Auto/KB/MB/GB units; Table preference to remember hidden columns and sort. Support reset-this-table and reset-all.
- Use the demo's narrow popovers, local feedback toasts, focus styles, Ctrl/Cmd+K sidebar search, Escape dismissal and mobile menu. The implementation must keep these interactions usable after HTMX fragment swaps.

## Exact interaction and data-contract matrix

| Reference interaction | Required production implementation |
| --- | --- |
| Select/expand schema or relation | Fetch scoped catalog nodes/rows with HTMX, preserve focus and visually selected tree row. |
| Simple / Advanced explorer | Same sidebar presentation and grouping as the demo. Advanced metadata objects are **inspection only**; never execute functions or procedures. |
| Search table names | Instant, permission-safe filtering with bounded server fallback for very large catalogs. |
| Select table, view or metadata object | Show corresponding compact toolbar and grid/structure/index/definition content; no fabricated records. |
| Search row contents | Server-side bounded filtering of the selected relation with honest limitations; never search only the current page while implying a table-wide match. |
| Column sort cycle | Ascending, descending, unsorted; preserve headers, align page state and validate allowed identifiers. |
| Row selection and inspection | Right-side details panel showing safe full/expanded values where supported. |
| Foreign-key link | Navigate to permitted referenced relation and scoped key lookup where supported. |
| Column picker | Show/hide columns and show-all/hide-all behavior, with table-scoped preference. |
| Compact and page-size controls | Change density or fetch 25/50/100 rows with correct range and next/previous state. |
| CSV view, CSV entire table, JSON entire table, copy CSV | Match labels/actions, but use explicit read-only authorization, limits and preferably streaming; avoid collecting full tables in JS. |
| Advanced linked-table export ZIP | Select relations, optionally include linked relations and relationship metadata; enforce scope/size/timeout constraints and export in bounded streams. |
| Bottom connection selector | Switch trusted configured profiles only; never expose raw URLs/passwords, infer a successful connection or accept arbitrary browser connection targets. |
| Preferences and reset controls | Small, immediate local state changes; persist only non-sensitive choices and per-table sort/hidden-column names. |
| Overview / routines / types / triggers / sequences | Read-only PostgreSQL metadata pages with same compact styling when the adapter supports them. |
| Loading / empty / errors | Use the same shell regions, not unrelated full-screen layouts; distinguish unsupported features clearly. |

## UI fidelity acceptance and verification

Use the **standalone HTML file itself** as the visual baseline. Capture or compare paired browser screenshots in the same browser engine and viewport at **1440×900**, **1280×800**, and **390×844**. Include light and dark themes, Simple and Advanced navigation, Data grid with/without row inspector, Structure, Indexes/Storage, Overview, connection picker, column/export/settings popovers, and mobile drawer. The expectation is faithful layout and styling rather than approximate resemblance; differences in spacing, type size, colors, component placement or behavior must be corrected, or explicitly recorded and approved if required by safety, real-data constraints or accessibility. Use realistic deterministic fixture data for comparison, and compare against the same state rather than arbitrary live rows.

Completion of a frontend task requires both visual parity for its relevant controls and functional parity with safe real backend behavior. Screenshots alone are insufficient: exercise focus/keyboard, filtering, sort/page transitions, slow and failed requests, theme persistence and escaped malicious values. Keep CuaDB's single Rust server, one published web port and no frontend JavaScript framework. The source of truth for task completion remains the managed task files under `.docs/todo/`; this document defines their shared fidelity standard.

## Frontend architecture
Askama renders a single explorer page and reusable HTML fragments. Tailwind provides layout, typography, consistent tokens and responsive states. A small custom grid CSS layer handles sticky headers, horizontal scrolling, high-density rows and resize behavior. HTMX handles browser-to-Rust navigation and partial updates; vanilla JavaScript handles immediate local actions such as sidebar toggling, resizing, copying and keyboard shortcuts. No React, Next.js, JSON-first state store or frontend build server is required.

## Navigation and data contracts
The sidebar shows trusted connection labels and, within each database, supported schemas or namespaces and relation types. Expanding a schema lazily requests a fragment. Selecting a table loads its first data page, the demo's Data / Structure / Indexes selector and separate Advanced Overview. Every server route validates relation identity, sort, requested columns and bounded page controls before issuing database requests. The URL may preserve non-sensitive navigation context but never connection credentials, cell values or raw SQL.

## Fragment behavior
Each fragment owns a stable target: connection state, schema subtree, data grid, or inspector content. Fast repeated clicks should cancel or serialize conflicting requests so older results never overwrite the current selection. Every fragment needs an empty, loading, permission-denied, unsupported-capability and timeout state. Server-rendered messages should be accessible and accurately describe what happened. The same view models must support a full-page reload without relying on previously cached JavaScript state.

## Grid rules
The records grid is semantic HTML and remains read-only. It shows actual column names, typed values, visible NULL markers, sortable headers and previous/next navigation using real continuation state. Display only a bounded page: 25 rows by default to match the reference, with explicit 50/100 options, never an entire table. Large JSON, arrays, bytea and spatial values are truncated with a safe detail view; copying a cell is a browser-only action. The UI must distinguish estimated row counts from exact counts and disclose when a table has no stable pagination key.

## Accessibility and verification
Use visible keyboard focus, aria-sort on active sortable headers, labeled buttons and appropriately announced loading or failure feedback. Test narrow viewports, long column names, Unicode names, many columns, dark/light themes, rapid sorting, stale HTMX responses and large-cell detail interactions. Keep event handlers delegated to persistent elements or explicitly reinitialized after swaps. No browser-only interaction should start a database query unnecessarily.
