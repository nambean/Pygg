# D12 release checks (ADR-012): first pass

> **Status:** Report · 2026-11-12 · checks of the custom window controls and the interface against SCREENS.md D12 · order: [ROADMAP](../ROADMAP.md) D4

ADR-012 keeps DownFetch's traffic-light window controls on Windows. That costs Windows' own caption buttons, so SCREENS.md D12 lists the checks that must pass before release. This pass covered what can be checked from the code and from the browser preview (`desktop-ui`, Vite on port 1431 with demo data; this run used port 1441 because another session held 1431). No installer was run and the packaged app was not started. Everything marked **native** needs the app on a real Windows desktop.

**Result:** 5 problems fixed; 1 failure that needs a native change; 1 decision for the owner; 9 checks still to do natively.

**Update, same day:** the failure (#4, snap layouts on hover) is fixed with a native overlay and checked in a debug build on Windows 11; see [Snap layouts](#snap-layouts-4).

## Checklist

| # | Check | Result | Notes |
| --- | --- | --- | --- |
| 1 | Close, minimize and maximize by mouse | **native** | The buttons call Tauri's `close`, `minimize` and `toggleMaximize`; the capabilities allow all three. The preview has no window. |
| 2 | Close, minimize and maximize by keyboard | **pass** (code) / **native** | They are real `<button>`s in the Tab order, and their glyphs show on focus. Alt+Space (the system menu) and Alt+F4 come from Windows, so check them natively. |
| 3 | Double-clicking the drag region maximizes and restores | **native** | The sidebar header, toolbar, title and spacer carry `data-tauri-drag-region`, which handles double-click with the `internal-toggle-maximize` permission the app grants. |
| 4 | Windows 11 snap layouts when hovering over maximize | **fixed** (debug build, 100% scaling) / **native** at other scales | Windows shows the snap flyout only for a window that answers `WM_NCHITTEST` with `HTMAXBUTTON` over its maximize button. The buttons are HTML inside WebView2, whose child windows cover the client area, so the main window's procedure never sees that hit test. An invisible native child window now sits over the green button and answers `HTMAXBUTTON` (`snap.rs`). Details and what is left: [Snap layouts](#snap-layouts-4). |
| 5 | `Win+Arrow`, `Win+Z` and dragging to a screen edge | **native** | These depend on the window keeping `WS_MAXIMIZEBOX` and `WS_THICKFRAME`. tao keeps them without decorations, but check natively. `Win+Z` is the keyboard way to reach snap layouts. |
| 6 | Each control has an accessible name and role | **fixed** | The green button was named "Zoom", the macOS term. Windows screen readers announce "Maximize" or "Restore" on native caption buttons, so it is now named for its state, which comes from `isMaximized` and is updated on resize (`WindowControls.tsx`). Close and Minimize were already right, inside a group named "Window". |
| 7 | Inactive-window and hover states match DownFetch | **pass** (code) / **native** | The same component and CSS as DownFetch: grey when the window loses focus, glyphs on hover. Compare side by side in the native app. |
| 8 | Hit targets usable at 100–200% scaling | **owner decision** | The lights are 12×12 CSS px with an 8 px gap, as in DownFetch and macOS. That is below WCAG 2.2's 24×24 px minimum target size (2.5.8, AA): the 20 px between centers is also under the spacing exception. A larger invisible hit area would need 12 px gaps, which changes the look the team chose. Scaling enlarges them in physical pixels but not relative to the other controls. |
| 9 | Display scaling: the window fits the screen | **fixed** | `fit_to_screen` shrank the window to the work area but never below 1024×640. At 175% or 200% on a 1920×1080 screen (1097×590 or 960×516 logical), or 125% on 1366×768, the window was larger than the screen. The minimum now gives way to the work area when the work area is smaller (`lib.rs`, with a test). |
| 10 | Display scaling: clipped text at 100/125/150/200% | **pass** | Scaling changes the CSS viewport, not the layout rules. Every screen and a task's detail were scanned at 1280×800, 1024×640 (the minimum) and 960×516 (1080p at 200%) for text cut by `overflow: hidden`, and for text past the window's edge. Nothing was clipped except intended ellipses with the full text in a `title`. At 960 px, the Usage ledger and the Connections models table scroll sideways inside their own boxes, which is acceptable. **Native:** Windows' "Text size" setting, and a real 200% monitor. |
| 11 | Keyboard: every screen reachable and usable | **fixed** | Every screen is in the sidebar's Tab order; all but Workspaces and Policies also have a shortcut (Ctrl+1…7 and Ctrl+,; Ctrl+Shift+L toggles the sidebar). Segmented controls are radio groups with arrow keys; sheets trap Tab and close with Escape. Two failures were found and fixed: (a) **the focus ring was invisible on most controls.** It was a `box-shadow` on `:focus-visible`, and any later rule giving a control its own shadow replaced it: `.button`, `.button.prominent` (New Task, Review, Save), `.text-input`, `.popup-button`, the selected segment and `.chart-column`. It is now an `outline` (`styles.css`). (b) **Closing a sheet left focus on the page body** instead of the button that opened it. The hook read the "previous" element after the sheet's `autoFocus` had moved, and gave focus back while the window was still `inert`. Both are fixed (`useSheetFocus.ts`, `ui.tsx`). Checked by pressing Tab through Tasks, and by opening and closing Connect Provider… from the keyboard. |
| 12 | Screen reader: names, roles, structure (from the accessibility tree) | **fixed** / **native** | Every screen has one `h1` (the toolbar title) and `h2` sections. The sidebar marks the current screen with `aria-current="page"`. Sheets are `role="dialog"` with `aria-modal` and a name. Spinners, saved and error notes are `status` or `alert` live regions. No control lacked a name (icon buttons have `aria-label`, decorative icons are `aria-hidden`). Fixed: the five data tables had no accessible name, and now carry their section's heading. **Native:** listen to the app with Narrator and NVDA, especially the live regions on Tasks and the approval sheet. |
| 13 | Reduced motion | **fixed** | The spinner, the switch and the row chevron already stopped animating under `prefers-reduced-motion`. Usage's "Requests" link still scrolled smoothly; it now jumps (`Usage.tsx`). Nothing else animates. |
| 14 | High contrast (Windows contrast themes, `forced-colors`) | **fixed** (code) / **native** | Windows' contrast themes drop box-shadows and replace backgrounds. So the old focus ring vanished, the window buttons became blank circles (their glyphs are hidden until hover), selected items and segments lost their only cue, and the switches and chart bars disappeared. A `@media (forced-colors: active)` block now gives controls system-colored borders, shows the window-button glyphs, and marks selections and switches with `Highlight`; the outline focus ring takes the system color. The browser preview cannot emulate forced colors, so **check it natively** with each contrast theme. `prefers-contrast: more` already raised separators and secondary text. |

## Snap layouts (#4)

**How it works.** Subclassing the main window cannot work here: WebView2's own child windows (another process) are under the pointer, so Windows asks them, not Pygg's window, and they cannot be subclassed. The fix follows `tauri-plugin-snap-layout` and `tauri-plugin-frame` (tauri-apps/tauri#4531 is open upstream). `tauri-plugin-decorum` instead presses `Win+Z` for the user, which moves focus and needs a key-simulation crate. No plugin was added: the overlay is one small module (`snap.rs`, with its tests) on the `windows` crate already in the tree (0.62, through tao and wry), with five Win32 features.

- `src-tauri/src/snap.rs`: the `window_snap_area` command places a child window of class `PyggSnapOverlay` (`WS_CHILD | WS_CLIPSIBLINGS`, no layered or transparent style, no brush, never paints) on top of the web view, over the button. It answers `WM_NCHITTEST` with `HTMAXBUTTON`. A press and release on it posts `SC_MAXIMIZE` or `SC_RESTORE` to the window. A right click does nothing, as on the page. Entering and leaving (`TrackMouseEvent`, non-client) send `window-snap-hover` to the page. Windows only; elsewhere the command does nothing.
- `WindowControls.tsx` (Windows only): sends the green button's rectangle in physical pixels (CSS pixels × `devicePixelRatio`, which covers display scaling and page zoom) on mount, on the button's resize, on window resize, on a change of text direction, and when `devicePixelRatio` changes (moving to a screen with other scaling). It sends nothing on unmount, which hides the overlay. The core rounds outwards and refuses a rectangle that is not finite, empty, or over 256 px a side, so a wrong value can never cover the page (unit tests).
- `styles.css`: `.pointer-on-zoom` shows the glyphs and active colors while the pointer is on the overlay, because the page no longer sees `:hover` there.

**Checked** in a debug build (`cargo build`, the Vite UI on another port, a scratch `PYGG_DATA_DIR` and another identifier, so the installed Pygg and its store were not touched), Windows 11 build 26200, 100% scaling, by moving the real pointer from a script:

- The overlay is a child of the Tauri window above `WRY_WEBVIEW`, exactly over the button (12×12 px at client 60,20). `WindowFromPoint` at the button's center returns it, and its hit test answers 9 (`HTMAXBUTTON`).
- Hovering opens the snap layouts flyout, both restored and maximized. All three glyphs show while hovering, and the green button stays visible (the overlay paints nothing).
- Clicking maximizes; clicking again restores. Each click toggled once (the page's own `onClick` does not also fire). After each change the overlay follows the button.

**Still to check natively:**

1. At 125%, 150% and 200% scaling, and after dragging the window to a monitor with other scaling: the flyout opens over the whole button, and not beside it.
2. Choosing a layout in the flyout snaps Pygg (the window's minimum width of 1024 px may keep it out of narrow zones; Microsoft asks for 500 effective px or less).
3. With the window inactive (grey lights), one click maximizes.
4. Known trade-offs: the browser tooltip "Maximize" no longer shows on mouse hover (the native flyout replaces it), and the pressed shade (`:active`) does not show on a mouse press. Keyboard, Narrator and NVDA still use the HTML button, unchanged.

## Fixed in this pass

- `styles.css`: the focus ring is an outline, so no control's own shadow can hide it; there is a `forced-colors` block.
- `WindowControls.tsx`: the green button is named Maximize or Restore, following the window's state.
- `useSheetFocus.ts`, `ui.tsx`: focus returns to the sheet's launcher on close.
- `lib.rs`: the minimum window size fits a heavily scaled or small screen.
- `Usage.tsx`: no smooth scrolling under reduced motion; the data tables in Usage, Clients, Connections and Automatic Routing have accessible names.

Strings: "Maximize" and "Restore" are new, in English inside `t()`. The Vietnamese entry for "Zoom" in `locales/vi/shell.ts` is no longer used; it can be replaced when translation is done.

## Still needs the native app, a real screen reader or the owner

1. **Snap layouts on hover** (#4): built and checked at 100% scaling; scaling, snapping into a zone and the inactive window are left (see [Snap layouts](#snap-layouts-4)).
2. **Target size of the window buttons** (#8): the owner decides between the DownFetch look and 24 px targets.
3. In the packaged app on Windows 11: #1, #2 (Alt+Space, Alt+F4), #3, #5, #7. Also a real 200% monitor and Windows' Text size setting (#10).
4. Narrator and NVDA (#12); each Windows contrast theme (#14).
