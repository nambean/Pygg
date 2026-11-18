# Pygg branding

**Product:** Pygg  
**Repository:** [nambean/Pygg](https://github.com/nambean/Pygg)  
**Tagline:** One Flow. Many Agents.

## Brand assets

| File | Purpose |
|---|---|
| `pygg-logo.svg` | Horizontal logo and slogan |
| `pygg-logo-dark.svg` | The same for dark backgrounds (transparent, light text; the README uses it in dark mode) |
| `pygg-icon.svg` | Scalable symbol |
| `pygg-icon-32.png` | Small UI/favicon |
| `pygg-icon-64.png` | Toolbar/app icon |
| `pygg-icon-128.png` | Medium icon |
| `pygg-icon-256.png` | High-resolution icon |
| `pygg-icon-512.png` | App/store icon |
| `pygg-logo-full.png` | Horizontal logo and slogan (PNG, 640x178) |
| `pygg.ico` | Windows favicon/icon (256x256 PNG-compressed entry) |
| `pygg-social-preview.png` | GitHub's social preview, 1280x640 (Settings → General → Social preview); rendered from `pygg-social-preview.html` with headless Edge at that window size |
| `pygg-demo-1080p.mp4` | The 36-second demo for X, LinkedIn and launch posts (1920x1080, silent); `docs/images/demo.gif` is the README's 960-wide copy. Made with HyperFrames from real CLI output and the browser preview's demo-data screenshots |
| `brand.json` | Names, palette, asset paths |

### Colors
- Light accent: **`#0B7A4B`** (`--accent` in `apps/desktop/src/styles.css`)
- Dark accent: **`#0D8551`**
- Dark mark/wordmark: `#111827`

Raster assets are derived from `pygg-icon.svg`. Run `python scripts/export-pygg-branding.py` after installing `cairosvg` and `Pillow` to regenerate the complete PNG icon scale, ICO, and horizontal logo PNG locally.

The app's icons (`apps/desktop/src-tauri/icons`) are generated from `pygg-icon.svg` with `pnpm tauri icon ../../branding/pygg-icon.svg` (run in `apps/desktop`; the Android and iOS sets it also writes are not kept). Checked 2026-11-11 on a debug build: the window is titled Pygg, the executable carries the Pygg icon, and `/health` reports `"app": "Pygg"`. The installer takes its name from `productName` (Pygg); it is checked on a clean machine in ROADMAP N9.

Kept on purpose: the identifier `app.smarttaskrouter.desktop` (it names the folder that holds the user's store) and the credential service name `SmartTaskRouter` (keys already stored under it must stay readable).
