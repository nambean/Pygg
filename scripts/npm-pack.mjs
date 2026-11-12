// Packs the npm packages for `pygg` (ROADMAP D2, ADR-030) from a release build, without publishing anything.
//
//   node scripts/npm-pack.mjs [--exe PATH] [--out DIR]
//
// Copies the release `pygg-cli.exe` (default apps/desktop/src-tauri/target/release/pygg-cli.exe) into
// packages/npm/win32-x64/bin, then runs `npm pack` for @pygg/win32-x64 and pygg into DIR (default dist/npm). The
// binary is never committed. Publishing is the owner's step: `npm publish --access public` for each tarball, after
// registering the `pygg` name and the `@pygg` scope.
import { execSync } from 'node:child_process';
import { copyFileSync, mkdirSync, existsSync, readFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
const flag = name => { const i = args.indexOf(name); return i >= 0 ? args[i + 1] : undefined; };
const exe = resolve(flag('--exe') ?? join(root, 'apps/desktop/src-tauri/target/release/pygg-cli.exe'));
const out = resolve(flag('--out') ?? join(root, 'dist/npm'));

if (!existsSync(exe)) {
  console.error(`No ${exe}: build the release first (pnpm tauri build in apps/desktop).`);
  process.exit(1);
}
const main = JSON.parse(readFileSync(join(root, 'packages/npm/pygg/package.json'), 'utf8'));
const platform = JSON.parse(readFileSync(join(root, 'packages/npm/win32-x64/package.json'), 'utf8'));
if (main.optionalDependencies[platform.name] !== platform.version || main.version !== platform.version) {
  console.error(`Versions differ: pygg ${main.version} wants ${platform.name} ${main.optionalDependencies[platform.name]}, which is ${platform.version}.`);
  process.exit(1);
}

mkdirSync(join(root, 'packages/npm/win32-x64/bin'), { recursive: true });
copyFileSync(exe, join(root, 'packages/npm/win32-x64/bin/pygg-cli.exe'));
mkdirSync(out, { recursive: true });
for (const dir of ['packages/npm/win32-x64', 'packages/npm/pygg']) {
  const name = execSync(`npm pack --pack-destination "${out}"`, { cwd: join(root, dir), encoding: 'utf8' }).trim().split('\n').pop();
  console.log(join(out, name));
}
