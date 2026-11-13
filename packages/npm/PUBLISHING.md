# Publishing `pygg` to npm

> **Status:** Current · developer preview, unsigned, Windows x64 only (owner, 2026-11-12) · ROADMAP D2b · ADR-030

Two packages: `@pygg/win32-x64` holds the native `pygg-cli.exe`, and `@pygg/cli` is the launcher that runs it as the `pygg` command. Publish the platform package first, since the launcher names it as an optional dependency. (npm refused the unscoped name `pygg` on 2026-11-12 as too close to `pg`, `pkg`, `pug` and others; names under our own scope are not checked that way.)

## Once: account, sign-in and scope

1. Create an npm account at <https://www.npmjs.com/signup> and confirm the e-mail.
2. Turn on two-factor authentication: Account → Two-Factor Authentication, with an authenticator app or a security key. npm asks for it when publishing.
3. Create the organization that owns the `@pygg` scope: <https://www.npmjs.com/org/create>, name `pygg`, the free plan (public packages only).
4. Sign in from a terminal: `npm login`. npm prints a link and opens the browser; sign in there, and the terminal finishes by itself. `npm whoami` then prints the account name.

## Each release

From the repository root, after a release build (`pnpm tauri build` in `apps/desktop`):

```powershell
node scripts/npm-pack.mjs
npm publish dist/npm/pygg-win32-x64-0.1.0-dev.1.tgz --access public --tag dev
npm publish dist/npm/pygg-cli-0.1.0-dev.1.tgz --access public --tag dev
```

`--tag dev` keeps a pre-release off `latest`, and current npm refuses a pre-release version without a tag. People install it with `npm install -g @pygg/cli@dev`, then run `pygg doctor`.

For the next release, raise `version` in both `package.json` files and the `optionalDependencies` entry in `pygg/package.json` (the launcher, `@pygg/cli`) together; `npm-pack.mjs` refuses versions that differ.

## Published

- **`0.1.0-dev.1` is prepared, not published** (2026-11-13): both `package.json` files carry it. It adds `pygg setup`, `pygg run claude`, `--version`, the start-here text of a bare `pygg`, grouped `--help`, and the launcher's own errors and `--launcher-info`. Before publishing: a release build, `node scripts/npm-pack.mjs`, then from an empty folder `npm install -g @pygg/cli@dev` and check `pygg --version`, `pygg doctor`, `pygg setup --help`, `pygg run --help`. Release note: "Adds coding-tool setup and the Claude Code launcher."

- `@pygg/win32-x64@0.1.0-dev.0`, 2026-11-12 (tags `dev` and `latest`, since it was the package's first version).
- `@pygg/cli@0.1.0-dev.0`, 2026-11-12 (tag `dev`). Installed back from the registry into an empty folder: npm pulled `@pygg/win32-x64` with it, `pygg doctor` found the running app (schema v19) and `pygg mcp` answered `initialize` and `tools/list`.
