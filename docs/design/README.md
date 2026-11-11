# Pygg UI/UX design

> **Status:** Historical · index of the original design sketches · reviewed 2026-11-11 · order: [ROADMAP](../ROADMAP.md) · built: [Implementation Status](../IMPLEMENTATION-STATUS.md)

> **Update 2026-11-11:** the app has an Overview and an Agents screen besides the six task destinations, and the screens these SVGs sketched are built; the SVGs are kept as the original concept only. Current layout: [SCREENS.md](SCREENS.md) and the root README.


**[SCREENS.md](SCREENS.md) is the current screen design.** It covers the shell, value conventions, tokens, every screen and sheet as ASCII wireframes, the reuse map from DownFetch components, and Phase 1 scope. Claude and Codex each designed the screens independently, then settled their differences in a second round.

## Old SVG wireframes (out of date)

- [Smart Routing](smart-routing.svg)
- [Providers](providers.svg)
- [Model Combos & Policies](model-combos.svg)

These predate plan v1.1 and SCREENS.md:
- They show 12 navigation items; the current layout is in SCREENS.md and the root README.
- They show separate "Task Breakdown" and "Routing Recommendation" panels; the design now uses one route list with reasons and exclusions.
- They use a web-dashboard style; the design now follows DownFetch's macOS-native token style.

They will be replaced by working screens in the app that use mock data, reviewed with the `apple-design` skill.

All UI data shown is synthetic. Model prices, quotas, health and latency must come from real provider data or local measurements at runtime.
