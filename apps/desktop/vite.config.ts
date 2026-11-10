import { defineConfig } from 'vite';

// The dev server runs on a fixed port that tauri.conf.json's devUrl points at (strictPort: fail instead of
// moving to another port). Nothing under src-tauri is served; watching it would stall Vite while a Rust
// build writes thousands of files into src-tauri/target.
export default defineConfig({
  clearScreen: false,
  server: {
    port: 1431,
    strictPort: true,
    watch: { ignored: ['**/src-tauri/**'] },
  },
});
