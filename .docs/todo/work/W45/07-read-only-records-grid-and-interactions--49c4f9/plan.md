---
{id: GRP-033BDB49C4F9,name: 07 — Read-only records grid and interactions,type: documentation,extra: {created_at: 2026-11-04T07:32:06.325Z}}
---

# 07 — Read-only records grid and interactions — Plan

## Implementation approach
Use semantic HTML tables with sticky headers, clear type indicators and horizontal scrolling. Let Rust retrieve each page and Askama render complete grid fragments; HTMX swaps only the grid container. Keep browser-side JS narrow: resizing columns, copy-to-clipboard and keyboard shortcuts. Render large strings or JSON in a bounded detail panel rather than expanding every cell by default.

## Risks and safeguards
Very wide tables, large JSON documents and stale HTMX responses can make simple pages feel heavy. The grid should never fetch entire relations in client memory or expose edit controls. Accessibility matters: sorting directions must be announced, focus should remain usable and keyboard controls must not trap navigation.

## Execution and verification
Work through the tasks in their listed order unless dependencies or test evidence justify another sequence. Before editing code, inspect the task using the CLI and check any blocking task links. Move exactly the selected task to in-progress, implement a coherent slice, add tests for errors and edge cases, and run Bun format/check/test plus focused live database checks where specified. Do not edit Task-managed IDs or status history manually. Use task comment for findings and task check for accepted items, and run task validate before recording completion. Any discovered work that falls outside the current task should be discussed and created as a separate detailed Task record rather than silently enlarging the scope.

## Deliverables
The group is accepted when its linked tasks are completed with evidence, its README or architecture documentation accurately reflects the implemented features, and the app remains usable with the existing shell and single Docker container constraints.

## Demo fidelity implementation plan

**Required reference:** `assets/cuadb-ui-demo.html` and `.docs/docs/frontend-interactions.md`. Reproduce the demo's grid and toolbar in a static real-template fixture first, then connect row paging, sort, selection, column toggles, inspector and exports to server responses. Use the prototype's specific control placements and densities but replace its simulated full-table operations with bounded Rust logic. Confirm screenshot parity and keyboard parity on several selected table states. At least 1440×900 and 390×844 comparisons must be recorded with light/dark states where applicable. The demo's example values are fixture data only; deliver server-backed, permission-aware read-only functionality. Any material visual exception must be documented and approved before group completion.
