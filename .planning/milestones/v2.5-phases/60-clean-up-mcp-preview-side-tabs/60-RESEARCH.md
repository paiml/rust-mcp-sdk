# Phase 60: Clean up mcp-preview side tabs - Research

**Researched:** 2026-03-22
**Domain:** Frontend HTML/CSS/JS -- single-file SPA modifications (mcp-preview DevTools panel)
**Confidence:** HIGH

## Summary

Phase 60 modifies `crates/mcp-preview/assets/index.html`, a single-file SPA of ~2,847 lines containing all HTML, CSS, and JS for the MCP Apps Preview DevTools panel. The work is entirely frontend: making the right-side DevTools panel resizable and collapsible, adding a global "Clear All" button, and removing the Console tab plus all associated code.

The existing code is well-structured with clear CSS custom properties for theming, `data-tab` attribute-driven tab switching, and per-tab clear/copy button handlers. The Console tab has tendrils throughout the file -- CSS classes (`.console-log`, `.console-entry`, `.console-time`), HTML markup (`tab-console` section), JS methods (`logConsole`), and an iframe-injected console interception script. All of these must be removed cleanly.

The three-panel layout uses flexbox (`.main { display: flex }`) with `.tool-panel` (300px fixed left), `.preview-area` (flex: 1 center), and `.devtools-panel` (350px fixed right). The resize feature requires converting the fixed right panel to a variable-width panel with a draggable left boundary. No external libraries are needed -- pure CSS/JS drag-to-resize is the standard approach for this pattern.

**Primary recommendation:** Execute in a single plan with three ordered work streams: (1) Console tab removal (cleanest first, reduces code before adding features), (2) panel resize/collapse with drag handle and toggle button, (3) global Clear All button addition.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** "Dev Tools" toggle button in the top-right header area to open/close the panel
- **D-02:** Draggable left boundary on the DevTools panel, matching browser DevTools drag behavior
- **D-03:** No minimum width -- dragging to zero fully collapses the panel (equivalent to closing)
- **D-04:** Panel starts open on launch (default 350px, same as current)
- **D-05:** No persistence -- panel resets to default open state on page reload
- **D-06:** Panel stays on the right side (left: tools, center: preview, right: devtools)
- **D-07:** Add a global "Clear All" button at the top of the DevTools panel that clears all tabs at once
- **D-08:** Keep per-tab Clear buttons as they are today (independent per-tab clearing)
- **D-09:** Copy buttons remain per-tab only (no global copy)
- **D-10:** Remove the Console tab entirely -- browser DevTools console is sufficient
- **D-11:** Remaining tabs after removal: Network, Events, Protocol, Bridge (4 tabs)
- **D-12:** Remove the console interception script injection from the widget iframe wrapper
- **D-13:** Remove all Console-related CSS, HTML, and JS (logConsole method, console-log container, etc.)

### Claude's Discretion
- Drag handle visual indicator style (subtle line, dots, or invisible with cursor change)
- Animation/transition for open/close toggle
- Where exactly to place the global "Clear All" button within the DevTools header area
- Whether the "Dev Tools" toggle button shows open/close state visually (icon change, text change)

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

## Standard Stack

No new libraries needed. This is pure HTML/CSS/JS work within an existing single-file SPA.

### Core
| Technology | Version | Purpose | Why Standard |
|------------|---------|---------|--------------|
| Vanilla JS | ES2020+ | Drag-to-resize, toggle, clear-all logic | Already used throughout the SPA; no framework dependency |
| CSS Custom Properties | N/A | Theming for drag handle, new buttons | Existing pattern in the file (light/dark mode) |
| CSS Flexbox | N/A | Panel layout | Existing `.main` layout is flexbox |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Vanilla drag-resize | CSS `resize: horizontal` | CSS `resize` only works on overflow containers and gives a tiny corner handle, not a full-edge drag -- wrong UX for browser DevTools feel |
| Vanilla drag-resize | Split.js or similar | Adds external dependency for something achievable in ~40 lines of JS |

## Architecture Patterns

### Existing File Structure
```
crates/mcp-preview/assets/index.html  (~2,847 lines, single SPA)
  Lines 1-1038:      CSS styles
  Lines 1039-1214:   HTML markup
  Lines 1215-2847:   JavaScript (PreviewRuntime class + initialization)
```

### Three-Panel Layout (Current)
```
.main (display: flex)
  |-- .tool-panel (width: 300px, fixed left)
  |-- .preview-area (flex: 1, center)
  |-- .devtools-panel (width: 350px, fixed right)
```

### Three-Panel Layout (After Phase 60)
```
.main (display: flex)
  |-- .tool-panel (width: 300px, fixed left)
  |-- .preview-area (flex: 1, center)
  |-- .devtools-resize-handle (width: ~6px, cursor: col-resize)
  |-- .devtools-panel (width: variable, default 350px, collapsible to 0)
```

### Pattern 1: Drag-to-Resize (Browser DevTools Style)
**What:** A thin vertical handle element between the preview area and devtools panel. Mouse drag events adjust the panel width dynamically.
**When to use:** When the panel should resize like browser DevTools (Chrome, Firefox).
**Example:**
```javascript
// Mousedown on handle starts resize
handle.addEventListener('mousedown', (e) => {
  e.preventDefault();
  const startX = e.clientX;
  const startWidth = panel.getBoundingClientRect().width;

  const onMouseMove = (e) => {
    // Dragging left increases panel width (panel is on right side)
    const delta = startX - e.clientX;
    const newWidth = Math.max(0, startWidth + delta);
    panel.style.width = newWidth + 'px';
    // If dragged to zero, treat as collapsed
    if (newWidth === 0) panel.classList.add('collapsed');
    else panel.classList.remove('collapsed');
  };

  const onMouseUp = () => {
    document.removeEventListener('mousemove', onMouseMove);
    document.removeEventListener('mouseup', onMouseUp);
  };

  document.addEventListener('mousemove', onMouseMove);
  document.addEventListener('mouseup', onMouseUp);
});
```

### Pattern 2: Toggle Button State
**What:** A button in the header that shows whether DevTools is open or closed, toggling the panel visibility.
**When to use:** D-01 requires this in the header area.
**Example:**
```javascript
toggleBtn.addEventListener('click', () => {
  if (panel.classList.contains('collapsed')) {
    panel.style.width = '350px';
    panel.classList.remove('collapsed');
  } else {
    panel.style.width = '0px';
    panel.classList.add('collapsed');
  }
});
```

### Pattern 3: Global Clear All
**What:** A single button that clears all four remaining tabs by invoking each tab's existing clear logic.
**When to use:** D-07 requires a global clear button.
**Example:**
```javascript
clearAllBtn.addEventListener('click', () => {
  // Reuse existing per-tab clear logic
  ['network', 'events', 'protocol', 'bridge'].forEach(tab => {
    const clearBtn = document.querySelector(`.clear-btn[data-clear="${tab}"]`);
    if (clearBtn) clearBtn.click();
  });
});
```

### Anti-Patterns to Avoid
- **CSS `resize` property for the panel:** Gives wrong UX -- tiny bottom-right corner handle instead of full-edge drag. Users expect browser-DevTools-style dragging.
- **Storing panel width in localStorage:** D-05 explicitly says no persistence -- panel resets on reload.
- **Setting min-width on the panel:** D-03 says no minimum width -- dragging to zero collapses the panel.
- **Leaving `logConsole` calls in JS after removing Console tab:** Each `this.logConsole(...)` call will throw since the method is removed. Must convert to `logNetwork()` or `logEvent()` calls, or remove entirely.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| N/A | N/A | N/A | All work is straightforward DOM manipulation within an existing SPA |

This phase has no "deceptively complex" problems. Drag-to-resize is ~40 lines, toggle is ~10 lines, global clear is ~10 lines. The real complexity is in the surgical removal of Console tab code across CSS, HTML, and JS.

## Common Pitfalls

### Pitfall 1: Orphaned logConsole Calls
**What goes wrong:** Removing the `logConsole()` method but leaving call sites throughout the JS causes runtime errors.
**Why it happens:** `logConsole` is called from 8 locations across the file (lines 2150, 2177, 2180, 2280, 2295, 2729, 2744, and the iframe-injected script at line 2490).
**How to avoid:** Before removing the method, decide the fate of each call site:
- Lines 2150, 2177, 2180 (executeTool errors): These are error notifications -- redirect to `logNetwork()` or display inline in the preview area
- Line 2280 (tool returned text): Informational -- redirect to `logNetwork()` or remove
- Line 2295 (callTool bridge): Informational -- redirect to `logEvent()` or `logNetwork()`
- Lines 2729, 2744 (ChatGPT mode calls): Redirect to `logNetwork()`
- Lines 2482-2494 (iframe console capture): Remove entirely per D-12
**Warning signs:** Any `this.logConsole` left in the file after the method is deleted.

### Pitfall 2: Console Tab as Default Active Tab
**What goes wrong:** The Console tab is currently the default active tab (line 1126: `class="devtools-tab active"`, line 1133: `class="devtools-section active"`). If removed without making another tab the default, no tab content is visible on load.
**Why it happens:** Console is first in the tab list and has the `active` class.
**How to avoid:** Make Network the new default active tab (first in the remaining list).
**Warning signs:** Loading mcp-preview and seeing a blank DevTools panel.

### Pitfall 3: Copy Button Console Handler in collectTabContent
**What goes wrong:** The `collectTabContent` method (line 2075) has a specific `console` case. If not removed, clicking Copy on a non-existent tab could cause errors.
**Why it happens:** The copy handler dispatches by tab name.
**How to avoid:** Remove the `if (tab === 'console')` branch from `collectTabContent`.
**Warning signs:** Any reference to `'console'` remaining in the clear/copy button handlers.

### Pitfall 4: Resize Handle Z-index and Event Capture
**What goes wrong:** The drag handle doesn't respond to clicks, or text selection activates during drag.
**Why it happens:** Other elements overlap the handle, or the browser's default text-selection behavior interferes with mousemove.
**How to avoid:** Set `user-select: none` on `body` during drag, use `e.preventDefault()` on mousedown, and ensure the handle has adequate z-index.
**Warning signs:** Cursor not changing to `col-resize` on hover, or text getting selected while dragging.

### Pitfall 5: Panel Collapse Not Hiding Content
**What goes wrong:** When the panel width is 0, the tab buttons and content still render, potentially overflowing or causing layout issues.
**Why it happens:** `overflow: hidden` needs to be on the panel, and the resize handle needs to still be visible even when panel is collapsed.
**How to avoid:** The `.devtools-panel` already has `overflow: hidden` (line 401). When collapsed, the handle remains a sibling element outside the panel. Ensure the toggle button state reflects the collapsed state.
**Warning signs:** Tab text or buttons visible when panel width is at 0.

### Pitfall 6: Clear Button Handler Has Console Branch
**What goes wrong:** The clear button event handler (lines 1964-1987) has an explicit `if (target === 'console')` branch. Leaving it is dead code.
**Why it happens:** The clear handler dispatches by `data-clear` attribute value.
**How to avoid:** Remove the `'console'` branch from the clear button handler along with the HTML.
**Warning signs:** Dead `console` branch remaining in the clear handler.

## Code Examples

### Current Console-Related Code to Remove

**CSS to remove (lines 439-457):**
```css
/* Console */
.console-log { ... }
.console-entry { ... }
.console-entry.log { ... }
.console-entry.warn { ... }
.console-entry.error { ... }
.console-time { ... }
```

**HTML to remove (lines 1126, 1133-1141):**
```html
<!-- Tab button -->
<button class="devtools-tab active" data-tab="console">Console</button>
<!-- Tab content section -->
<div class="devtools-section active" id="tab-console">
  <div class="devtools-btn-group">
    <button class="copy-btn" data-copy="console">Copy</button>
    <button class="clear-btn" data-clear="console">Clear</button>
  </div>
  <div class="console-log" id="console-log">
    <div class="empty-state">Console output will appear here</div>
  </div>
</div>
```

**JS to remove/modify:**
- `logConsole()` method (lines 2641-2653)
- `collectTabContent` console branch (line 2075)
- Clear handler console branch (lines 1968-1970)
- Console capture script injection (lines 2482-2494)
- All `this.logConsole(...)` call sites (8 locations)

### Recommended logConsole Replacement Strategy

| Original Call | Location | Replacement |
|---------------|----------|-------------|
| `this.logConsole('error', \`Invalid JSON: ...\`)` | executeTool | `logEvent('error', { message: ... })` or inline UI error |
| `this.logConsole('error', result.error)` | executeTool | `logEvent('error', { message: result.error })` |
| `this.logConsole('error', \`Request failed: ...\`)` | executeTool | Already followed by `logNetwork` -- remove duplicate |
| `this.logConsole('log', \`Tool returned: ...\`)` | handleToolResponse | `logEvent('toolTextResponse', { text: ... })` |
| `this.logConsole('log', 'callTool: ' + name)` | createToolCallHandler | Already has `logEvent('bridgeCall', ...)` -- remove duplicate |
| `this.logConsole('log', '[ChatGPT] callTool...')` | handleWidgetToolCall | Already has `logEvent('chatgptCallTool', ...)` -- remove duplicate |
| `this.logConsole('error', '[ChatGPT] callTool failed...')` | handleWidgetToolCall | `logEvent('chatgptCallToolError', { message: ... })` |
| `preview.logConsole(method, ...)` | iframe injection | Remove entire script block per D-12 |

### Drag Handle CSS Pattern
```css
.devtools-resize-handle {
  width: 6px;
  cursor: col-resize;
  background: transparent;
  position: relative;
  flex-shrink: 0;
}

.devtools-resize-handle:hover,
.devtools-resize-handle.dragging {
  background: var(--accent-color);
  opacity: 0.3;
}
```

### Toggle Button CSS Pattern
```css
.devtools-toggle {
  padding: 6px 12px;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  border-radius: 4px;
  font-size: 13px;
}

.devtools-toggle.panel-open {
  color: var(--accent-color);
}
```

### Global Clear All Button Placement
Recommended: Place in the `.devtools-tabs` bar, right-aligned, alongside tab buttons. This keeps it associated with the DevTools panel content (not the header).
```html
<div class="devtools-tabs">
  <button class="devtools-tab active" data-tab="network">Network</button>
  <button class="devtools-tab" data-tab="events">Events</button>
  <button class="devtools-tab" data-tab="protocol">Protocol</button>
  <button class="devtools-tab" data-tab="bridge">Bridge</button>
  <button class="devtools-clear-all" id="clear-all-btn" title="Clear All">Clear All</button>
</div>
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Fixed-width DevTools panel | Resizable panels (all modern browsers) | Standard since Chrome DevTools inception | Users expect drag-to-resize for dev tool panels |
| Console tab in custom DevTools | Browser DevTools console | Always available | Removes maintenance burden; browser console is richer and more reliable |

## Open Questions

1. **What should happen to logConsole error calls in executeTool?**
   - What we know: There are 3 error logging calls in `executeTool()` that use `logConsole('error', ...)`. Two of them (request failure, tool error) are already accompanied by `logNetwork()` calls.
   - What's unclear: Whether the single `Invalid JSON` error (line 2150) should become an event, a network entry, or just be ignored (user sees the JSON parse error in their args editor context anyway).
   - Recommendation: Route to `logEvent('error', ...)` for the Invalid JSON case; the other two are already covered by network logging and can be removed as duplicates.

2. **Should the drag handle be visible at rest or only on hover?**
   - What we know: Browser DevTools (Chrome, Firefox) use a very subtle approach -- cursor changes to `col-resize` on hover, no visible indicator at rest.
   - What's unclear: Whether first-time users will discover the resize capability.
   - Recommendation: Invisible at rest with `col-resize` cursor, and a subtle highlight (var(--accent-color) at low opacity) on hover. This matches Chrome DevTools exactly.

## Validation Architecture

No test framework applies here -- this is a self-contained HTML/CSS/JS file served as a static asset. Validation is manual:

| Req | Behavior | Test Type | Validation |
|-----|----------|-----------|------------|
| D-01 | Toggle button opens/closes panel | manual | Click Dev Tools button in header; verify panel toggles |
| D-02 | Draggable left boundary | manual | Drag the resize handle; verify panel width changes |
| D-03 | No minimum width | manual | Drag to zero width; verify panel fully collapses |
| D-04 | Default 350px open | manual | Reload page; verify panel is 350px wide |
| D-07 | Global Clear All | manual | Add entries to tabs, click Clear All, verify all tabs cleared |
| D-08 | Per-tab clear preserved | manual | Add entries, clear individual tab, verify only that tab cleared |
| D-10 | Console tab removed | manual | Verify only 4 tabs visible: Network, Events, Protocol, Bridge |
| D-12 | No iframe console capture | manual | Open browser DevTools, verify widget console.log appears there (not in custom panel) |
| D-13 | No console CSS/HTML/JS remnants | grep audit | Search for `console-log`, `console-entry`, `logConsole`, `tab-console` in index.html |

### Automated Grep Audit (Post-Implementation)
```bash
# Should return zero matches for Console-related artifacts
grep -c 'console-log\|console-entry\|console-time\|logConsole\|tab-console\|data-tab="console"\|data-clear="console"\|data-copy="console"' crates/mcp-preview/assets/index.html
```

Note: `console.log`/`console.error` (native browser API calls) are expected to remain -- only the custom `logConsole` method and Console tab UI should be removed.

### Build Verification
```bash
# Ensure the Rust crate still compiles (index.html is embedded via include_str!)
cargo build -p mcp-preview
```

## Sources

### Primary (HIGH confidence)
- `crates/mcp-preview/assets/index.html` -- direct code inspection of the 2,847-line SPA
- `.planning/phases/60-clean-up-mcp-preview-side-tabs/60-CONTEXT.md` -- all 13 locked decisions

### Secondary (MEDIUM confidence)
- `.planning/phases/44-improving-mcp-preview-to-support-chatgpt-version/44-CONTEXT.md` -- Phase 44 established the 5-tab DevTools panel structure
- Chrome DevTools UX -- drag-to-resize behavior is the standard reference for D-02

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no external dependencies, pure vanilla JS/CSS
- Architecture: HIGH -- direct code inspection of the single implementation file
- Pitfalls: HIGH -- all identified via line-by-line tracing of Console references through the codebase

**Research date:** 2026-03-22
**Valid until:** 2026-04-22 (stable -- this is internal UI code with no external API dependencies)
