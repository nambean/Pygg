# D12 release checks (ADR-012): first pass

> **Status:** Report · 2026-11-12 · checks of the custom window controls and the interface against SCREENS.md D12 · order: [ROADMAP](../ROADMAP.md) D4

ADR-012 keeps DownFetch's traffic-light window controls on Windows. That costs Windows' own caption buttons, so SCREENS.md D12 lists the checks that must pass before release. This pass covered what can be checked from the code and from the browser preview (`desktop-ui`, Vite on port 1431 with demo data; this run used port 1441 because another session held 1431). No installer was run and the packaged app was not started. Everything marked **native** needs the app on a real Windows desktop.

**Result:** 5 problems fixed; 1 failure that needs a native change; 1 decision for the owner; 9 checks still to do natively.

## Checklist

| # | Check | Result | Notes |
| --- | --- | --- | --- |
| 1 | Close, minimize and maximize by mouse | **native** | The buttons call Tauri's `close`, `minimize` and `toggleMaximize`; the capabilities allow all three. The preview has no window. |
| 2 | Close, minimize and maximize by keyboard | **pass** (code) / **native** | They are real `<button>`s in the Tab order, and their glyphs show on focus. Alt+Space (the system menu) and Alt+F4 come from Windows, so check them natively. |
| 3 | Double-clicking the drag region maximizes and restores | **native** | The sidebar header, toolbar, title and spacer carry `data-tauri-drag-region`, which handles double-click with the `internal-toggle-maximize` permission the app grants. |
| 4 | Windows 11 snap layouts when hovering over maximize | **fail** | Windows shows the snap flyout only for a window that answers `WM_NCHITTEST` with `HTMAXBUTTON` over its maximize button. The window has `decorations: false`, and its buttons are HTML, so there is no flyout. **Needs a native change:** subclass the window procedure to return `HTMAXBUTTON` over the green button's rectangle (the interface sends that rectangle when it changes), or adopt a plugin that does this. This is not done here: it can only be built and checked in the running app. |
| 5 | `Win+Arrow`, `Win+Z` and dragging to a screen edge | **native** | These depend on the window keeping `WS_MAXIMIZEBOX` and `WS_THICKFRAME`. tao keeps them without decorations, but check natively. `Win+Z` is the keyboard way to reach snap layouts while #4 fails. |
| 6 | Each control has an accessible name and role | **fixed** | The green button was named "Zoom", the macOS term. Windows screen readers announce "Maximize" or "Restore" on native caption buttons, so it is now named for its state, which comes from `isMaximized` and is updated on resize (`WindowControls.tsx`). Close and Minimize were already right, inside a group named "Window". |
| 7 | Inactive-window and hover states match DownFetch | **pass** (code) / **native** | The same component and CSS as DownFetch: grey when the window loses focus, glyphs on hover. Compare side by side in the native app. |
| 8 | Hit targets usable at 100–200% scaling | **owner decision** | The lights are 12×12 CSS px with an 8 px gap, as in DownFetch and macOS. That is below WCAG 2.2's 24×24 px minimum target size (2.5.8, AA): the 20 px between centers is also under the spacing exception. A larger invisible hit area would need 12 px gaps, which changes the look the team chose. Scaling enlarges them in physical pixels but not relative to the other controls. |
| 9 | Display scaling: the window fits the screen | **fixed** | `fit_to_screen` shrank the window to the work area but never below 1024×640. At 175% or 200% on a 1920×1080 screen (1097×590 or 960×516 logical), or 125% on 1366×768, the window was larger than the screen. The minimum now gives way to the work area when the work area is smaller (`lib.rs`, with a test). |
| 10 | Display scaling: clipped text at 100/125/150/200% | **pass** | Scaling changes the CSS viewport, not the layout rules. Every screen and a task's detail were scanned at 1280×800, 1024×640 (the minimum) and 960×516 (1080p at 200%) for text cut by `overflow: hidden`, and for text past the window's edge. Nothing was clipped except intended ellipses with the full text in a `title`. At 960 px, the Usage ledger and the Connections models table scroll sideways inside their own boxes, which is acceptable. **Native:** Windows' "Text size" setting, and a real 200% monitor. |
| 11 | Keyboard: every screen reachable and usable | **fixed** | Every screen is in the sidebar's Tab order; all but Workspaces and Policies also have a shortcut (Ctrl+1…7 and Ctrl+,; Ctrl+Shift+L toggles the sidebar). Segmented controls are radio groups with arrow keys; sheets trap Tab and close with Escape. Two failures were found and fixed: (a) **the focus ring was invisible on most controls.** It was a `box-shadow` on `:focus-visible`, and any later rule giving a control its own shadow replaced it: `.button`, `.button.prominent` (New Task, Review, Save), `.text-input`, `.popup-button`, the selected segment and `.chart-column`. It is now an `outline` (`styles.css`). (b) **Closing a sheet left focus on the page body** instead of the button that opened it. The hook read the "previous" element after the sheet's `autoFocus` had moved, and gave focus back while the window was still `inert`. Both are fixed (`useSheetFocus.ts`, `ui.tsx`). Checked by pressing Tab through Tasks, and by opening and closing Connect Provider… from the keyboard. |
| 12 | Screen reader: names, roles, structure (from the accessibility tree) | **fixed** / **native** | Every screen has one `h1` (the toolbar title) and `h2` sections. The sidebar marks the current screen with `aria-current="page"`. Sheets are `role="dialog"` with `aria-modal` and a name. Spinners, saved and error notes are `status` or `alert` live regions. No control lacked a name (icon buttons have `aria-label`, decorative icons are `aria-hidden`). Fixed: the five data tables had no accessible name, and now carry their section's heading. **Native:** listen to the app with Narrator and NVDA, especially the live regions on Tasks and the approval sheet. |
| 13 | Reduced motion | **fixed** | The spinner, the switch and the row chevron already stopped animating under `prefers-reduced-motion`. Usage's "Requests" link still scrolled smoothly; it now jumps (`Usage.tsx`). Nothing else animates. |
| 14 | High contrast (Windows contrast themes, `forced-colors`) | **fixed** (code) / **native** | Windows' contrast themes drop box-shadows and replace backgrounds. So the old focus ring vanished, the window buttons became blank circles (their glyphs are hidden until hover), selected items and segments lost their only cue, and the switches and chart bars disappeared. A `@media (forced-colors: active)` block now gives controls system-colored borders, shows the window-button glyphs, and marks selections and switches with `Highlight`; the outline focus ring takes the system color. The browser preview cannot emulate forced colors, so **check it natively** with each contrast theme. `prefers-contrast: more` already raised separators and secondary text. |

## Fixed in this pass

- `styles.css`: the focus ring is an outline, so no control's own shadow can hide it; there is a `forced-colors` block.
- `WindowControls.tsx`: the green button is named Maximize or Restore, following the window's state.
- `useSheetFocus.ts`, `ui.tsx`: focus returns to the sheet's launcher on close.
- `lib.rs`: the minimum window size fits a heavily scaled or small screen.
- `Usage.tsx`: no smooth scrolling under reduced motion; the data tables in Usage, Clients, Connections and Automatic Routing have accessible names.

Strings: "Maximize" and "Restore" are new, in English inside `t()`. The Vietnamese entry for "Zoom" in `locales/vi/shell.ts` is no longer used; it can be replaced when translation is done.

## Still needs the native app, a real screen reader or the owner

1. **Snap layouts on hover** (#4): a native `WM_NCHITTEST` change, then a check on Windows 11.
2. **Target size of the window buttons** (#8): the owner decides between the DownFetch look and 24 px targets.
3. In the packaged app on Windows 11: #1, #2 (Alt+Space, Alt+F4), #3, #5, #7. Also a real 200% monitor and Windows' Text size setting (#10).
4. Narrator and NVDA (#12); each Windows contrast theme (#14).
