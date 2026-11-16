// Packs the npm packages for the `pygg` command (ADR-037): the launcher @pygg/cli, and one package per platform holding
// the native `pygg-cli` program (@pygg/win32-x64, @pygg/darwin-arm64, @pygg/darwin-x64, @pygg/linux-x64). Nothing is
// published: that is the owner's step (packages/npm/PUBLISHING.md).
//
//   node scripts/npm-pack.mjs --platform linux-x64 --exe PATH [--out DIR]   one platform package, from a built program
//   node scripts/npm-pack.mjs --launcher [--out DIR]                         the launcher only
//   node scripts/npm-pack.mjs [--exe PATH] [--out DIR]                       this computer's platform and the launcher
//                                                                            (default program: the release build)
//
// A platform package is generated (its package.json comes from PLATFORMS below, at the launcher's version) in
// DIR/stage/<platform>, so no binary is ever committed. Pack each platform on its own kind of computer (CI does): npm
// keeps the program's execute bit only when the file system has one. The launcher's optional dependencies must name
// every platform here at its own version, or nothing is packed.
import { execFileSync, execSync } from 'node:child_process';
import { chmodSync, copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/** npm's `<os>-<cpu>` name → the Rust target CI builds it with, and npm's install filters. */
export const PLATFORMS = {
  'win32-x64': { rust: 'x86_64-pc-windows-msvc', os: 'win32', cpu: 'x64', exe: 'pygg-cli.exe' },
  'darwin-arm64': { rust: 'aarch64-apple-darwin', os: 'darwin', cpu: 'arm64', exe: 'pygg-cli' },
  'darwin-x64': { rust: 'x86_64-apple-darwin', os: 'darwin', cpu: 'x64', exe: 'pygg-cli' },
  'linux-x64': { rust: 'x86_64-unknown-linux-gnu', os: 'linux', cpu: 'x64', libc: 'glibc', exe: 'pygg-cli' },
};

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
const flag = name => { const i = args.indexOf(name); return i >= 0 ? args[i + 1] : undefined; };
const out = resolve(flag('--out') ?? join(root, 'dist/npm'));
const launcherDir = join(root, 'packages/npm/pygg');
const launcher = JSON.parse(readFileSync(join(launcherDir, 'package.json'), 'utf8'));

function die(message) {
  console.error(`npm-pack: ${message}`);
  process.exit(1);
}

function checkVersions() {
  const wanted = Object.keys(PLATFORMS).map(p => `@pygg/${p}`).sort();
  const named = Object.keys(launcher.optionalDependencies ?? {}).sort();
  if (JSON.stringify(wanted) !== JSON.stringify(named)) {
    die(`@pygg/cli's optionalDependencies name ${named.join(', ') || 'nothing'}; they must be exactly ${wanted.join(', ')}.`);
  }
  for (const [name, version] of Object.entries(launcher.optionalDependencies)) {
    if (version !== launcher.version) die(`@pygg/cli ${launcher.version} wants ${name} ${version}; both must be the same version.`);
  }
}

function pack(dir) {
  mkdirSync(out, { recursive: true });
  const output = execSync(`npm pack --pack-destination "${out}"`, { cwd: dir, encoding: 'utf8' });
  const file = join(out, output.trim().split('\n').pop().trim());
  console.log(file);
  return file;
}

/** The platform package's package.json. */
export function manifest(platform, version) {
  const p = PLATFORMS[platform];
  return {
    name: `@pygg/${platform}`,
    version,
    description: `The native pygg-cli program for ${platform}, used by @pygg/cli (the pygg command). Developer preview, unsigned.`,
    license: 'MIT',
    repository: { type: 'git', url: 'git+https://github.com/nambean/Pygg.git' },
    homepage: 'https://github.com/nambean/Pygg#readme',
    bugs: { url: 'https://github.com/nambean/Pygg/issues' },
    os: [p.os],
    cpu: [p.cpu],
    ...(p.libc ? { libc: [p.libc] } : {}),
    files: ['bin/'],
    preferUnplugged: true,
  };
}

function packPlatform(platform, exe) {
  const p = PLATFORMS[platform];
  if (!p) die(`no platform ${platform}; one of ${Object.keys(PLATFORMS).join(', ')}.`);
  if (!existsSync(exe)) die(`no program at ${exe}: build it first (cargo build --release --no-default-features --bin pygg-cli).`);
  // The program reports the launcher's version (build.rs reads it from package.json); an older build would not. Checked
  // where the program can run, so a cross-built one (the Intel Mac build on Apple silicon) is not.
  if (platform === `${process.platform}-${process.arch}`) {
    const reports = execFileSync(exe, ['--version'], { encoding: 'utf8' }).trim();
    if (reports !== `pygg-cli ${launcher.version}`) die(`${exe} reports "${reports}", not pygg-cli ${launcher.version}: rebuild it.`);
  }
  const stage = join(out, 'stage', platform);
  rmSync(stage, { recursive: true, force: true });
  mkdirSync(join(stage, 'bin'), { recursive: true });
  writeFileSync(join(stage, 'package.json'), `${JSON.stringify(manifest(platform, launcher.version), null, 2)}\n`);
  writeFileSync(join(stage, 'README.md'), `# @pygg/${platform}\n\nThe native \`pygg-cli\` program for ${platform}. Install \`@pygg/cli\` instead: it brings the package for your computer and runs it as \`pygg\`.\n\nPygg is open source (MIT) at [github.com/nambean/Pygg](https://github.com/nambean/Pygg): issues, ideas and pull requests are welcome.\n`);
  copyFileSync(join(root, 'LICENSE'), join(stage, 'LICENSE'));
  const target = join(stage, 'bin', p.exe);
  copyFileSync(exe, target);
  if (p.os !== 'win32') chmodSync(target, 0o755);
  return pack(stage);
}

if (!args.includes('--no-version-check')) checkVersions();
if (args.includes('--launcher')) {
  pack(launcherDir);
} else if (flag('--platform')) {
  packPlatform(flag('--platform'), resolve(flag('--exe') ?? ''));
} else {
  const here = `${process.platform}-${process.arch}`;
  if (!PLATFORMS[here]) die(`no package for this computer (${here}); pass --platform and --exe.`);
  const exe = resolve(flag('--exe') ?? join(root, 'apps/desktop/src-tauri/target/release', PLATFORMS[here].exe));
  packPlatform(here, exe);
  pack(launcherDir);
}
