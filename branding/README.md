# Pygg branding

**Product:** Pygg  
**Engineering repository:** SmartTaskRouter  
**Tagline:** One Flow. Many Agents.

## Brand assets

| File | Purpose |
|---|---|
| `pygg-logo.svg` | Horizontal logo and slogan |
| `pygg-icon.svg` | Scalable symbol |
| `pygg-icon-32.png` | Small UI/favicon |
| `pygg-icon-64.png` | Toolbar/app icon |
| `pygg-icon-128.png` | Medium icon |
| `pygg-icon-256.png` | High-resolution icon |
| `pygg.ico` | Windows favicon/icon (256x256 PNG-compressed entry) |
| `brand.json` | Names, palette, asset paths |

### Colors
- Light accent: **`#0B7A4B`** (`--accent` in `apps/desktop/src/styles.css`)
- Dark accent: **`#0D8551`**
- Dark mark/wordmark: `#111827`

Raster assets are derived from `pygg-icon.svg`. Run `python scripts/export-pygg-branding.py` after installing `cairosvg` and `Pillow` to regenerate the complete PNG icon scale, ICO, and horizontal logo PNG locally.

These brand exports are not automatically wired into the existing Tauri runtime icons; that integration requires its own packaging verification.
