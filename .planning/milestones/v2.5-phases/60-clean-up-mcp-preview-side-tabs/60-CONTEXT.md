# Phase 60: Clean up mcp-preview side tabs - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Clean up the mcp-preview DevTools side panel: make it resizable and fully collapsible, add a global clear-all button while keeping per-tab clear, and remove the Console tab. The tool panel (left) and preview area (center) are untouched.

</domain>

<decisions>
## Implementation Decisions

### Panel resize and collapse
- **D-01:** "Dev Tools" toggle button in the top-right header area to open/close the panel
- **D-02:** Draggable left boundary on the DevTools panel, matching browser DevTools drag behavior
- **D-03:** No minimum width — dragging to zero fully collapses the panel (equivalent to closing)
- **D-04:** Panel starts open on launch (default 350px, same as current)
- **D-05:** No persistence — panel resets to default open state on page reload
- **D-06:** Panel stays on the right side (left: tools, center: preview, right: devtools)

### Clear button consolidation
- **D-07:** Add a global "Clear All" button at the top of the DevTools panel that clears all tabs at once
- **D-08:** Keep per-tab Clear buttons as they are today (independent per-tab clearing)
- **D-09:** Copy buttons remain per-tab only (no global copy)

### Console tab removal
- **D-10:** Remove the Console tab entirely — browser DevTools console is sufficient
- **D-11:** Remaining tabs after removal: Network, Events, Protocol, Bridge (4 tabs)
- **D-12:** Remove the console interception script injection from the widget iframe wrapper
- **D-13:** Remove all Console-related CSS, HTML, and JS (logConsole method, console-log container, etc.)

### Claude's Discretion
- Drag handle visual indicator style (subtle line, dots, or invisible with cursor change)
- Animation/transition for open/close toggle
- Where exactly to place the global "Clear All" button within the DevTools header area
- Whether the "Dev Tools" toggle button shows open/close state visually (icon change, text change)

</decisions>

<specifics>
## Specific Ideas

- Panel resize should feel like browser DevTools — familiar drag-to-resize UX
- The "Dev Tools" button doubles as a toggle: clicking when open closes the panel, clicking when closed opens it

</specifics>

<canonical_refs>
## Canonical References

No external specs — requirements are fully captured in decisions above.

### Prior mcp-preview work
- `.planning/phases/44-improving-mcp-preview-to-support-chatgpt-version/44-CONTEXT.md` — Phase 44 added the Protocol tab and established the 5-tab DevTools panel structure

### Implementation files
- `crates/mcp-preview/assets/index.html` — Single-file SPA (~2,847 lines) containing all HTML, CSS, and JS for the DevTools panel
- Lines 404-437: DevTools panel CSS (350px fixed width, tab styling)
- Lines 743-765: Clear/Copy button group CSS
- Lines 1125-1131: Tab button HTML markup (Console, Network, Events, Protocol, Bridge)
- Lines 1134-1172: Per-tab content sections with clear/copy button groups
- Lines 1965-1987: Clear button JavaScript handler (per-tab clearing logic)
- Lines 2482-2494: Console interception script injected into widget iframe
- Lines 2641-2653: logConsole() method rendering console entries

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- Existing `.devtools-tabs` flexbox layout — will have one fewer tab button
- Existing `.clear-btn` and `.copy-btn` CSS classes — reusable for global clear button styling
- Per-tab clear handler in JS already knows how to clear each tab's data — global clear can call each

### Established Patterns
- CSS custom properties for theming (light/dark mode) — drag handle should use existing variables
- Button group pattern (`.devtools-btn-group`) — reuse for global clear placement
- Tab switching via `data-tab` attributes and classList toggle

### Integration Points
- Header bar (top of page) — add "Dev Tools" toggle button
- `.devtools-panel` CSS — change from fixed 350px to resizable with drag handle
- `.devtools-tabs` container — add global clear button alongside tab buttons
- Widget iframe wrapper script — remove console interception injection
- PreviewRuntime class — remove logConsole() method and console-related state

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 60-clean-up-mcp-preview-side-tabs*
*Context gathered: 2026-03-22*
