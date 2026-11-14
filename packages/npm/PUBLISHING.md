# Publishing `pygg` to npm

> **Status:** Current · developer preview, unsigned · Windows x64, macOS (Apple silicon and Intel) and Linux x64 (glibc) since 2026-11-14 (ADR-037) · ROADMAP D2b · ADR-030

Five packages. `@pygg/cli` is the launcher that runs the native program as the `pygg` command; `@pygg/win32-x64`, `@pygg/darwin-arm64`, `@pygg/darwin-x64` and `@pygg/linux-x64` each hold the native `pygg-cli` for one platform, and npm installs only the one that matches the computer (they are the launcher's optional dependencies, restricted by `os`, `cpu` and `libc`). Publish the four platform packages first, since the launcher names them. (npm refused the unscoped name `pygg` on 2026-11-12 as too close to `pg`, `pkg`, `pug` and others; names under our own scope are not checked that way.)

## Once: account, sign-in and scope

1. Create an npm account at <https://www.npmjs.com/signup> and confirm the e-mail.
2. Turn on two-factor authentication: Account → Two-Factor Authentication, with an authenticator app or a security key. npm asks for it when publishing.
3. Create the organization that owns the `@pygg` scope: <https://www.npmjs.com/org/create>, name `pygg`, the free plan (public packages only).
4. Sign in from a terminal: `npm login`. npm prints a link and opens the browser; sign in there, and the terminal finishes by itself. `npm whoami` then prints the account name.

## Each release

The packages are built by CI, not on the owner's PC: the macOS and Linux programs can only be built there.

1. Raise `version` in `packages/npm/pygg/package.json` and the four `optionalDependencies` entries together (`npm-pack.mjs` refuses versions that differ). The platform packages take the launcher's version.
2. Push to `main` (or run **npm packages** from the Actions tab). The workflow builds `pygg-cli` on Windows, Linux and macOS, packs every package, installs each platform's back from the tarballs and runs `pygg`. Download the artifact **npm-packages-all** (five `.tgz` files) from the run.
3. Publish, from the folder holding the tarballs, platform packages first:

```powershell
npm publish pygg-win32-x64-0.1.0-dev.1.tgz --access public --tag dev
npm publish pygg-darwin-arm64-0.1.0-dev.1.tgz --access public --tag dev
npm publish pygg-darwin-x64-0.1.0-dev.1.tgz --access public --tag dev
npm publish pygg-linux-x64-0.1.0-dev.1.tgz --access public --tag dev
npm publish pygg-cli-0.1.0-dev.1.tgz --access public --tag dev
```

`--tag dev` keeps a pre-release off `latest`, and current npm refuses a pre-release version without a tag. npm asks for the two-factor code at each publish. People install it with `npm install -g @pygg/cli@dev`, then run `pygg`.

4. Check from a clean folder on each system you have: `npm install -g @pygg/cli@dev`, `pygg --launcher-info` (names the platform package it runs), `pygg --version`, `pygg doctor`.

A Windows-only package can still be made locally after a release build: `node scripts/npm-pack.mjs` packs this computer's platform package and the launcher into `dist/npm`. Publishing that alone leaves the other platforms' optional dependencies missing, which the launcher reports on those systems.

## Published

- **`0.1.0-dev.1`, 2026-11-14 (tag `dev`): `@pygg/cli`, `@pygg/win32-x64` and `@pygg/linux-x64` published; `@pygg/darwin-arm64` and `@pygg/darwin-x64` not yet.** CI could not run that day (GitHub Actions refused every job over the account's billing), so the Windows program was built on the owner's PC and the Linux one in Docker (`rust:1-bookworm`, packed in `node:22-bookworm-slim` so the program keeps its execute bit), then published from `dist/npm-release` with web two-factor sign-in. npm skips optional dependencies that are not in the registry, so installing works without the macOS packages; on a Mac the launcher says no program is published until they are (same version, built by CI once it runs). Installed back from the registry into empty places: Windows (`--launcher-info` finds `@pygg/win32-x64`, `--version`, a mistyped command suggests `pygg status` and exits 2, `pygg doctor` reads the running app's store) and Linux (`npm install -g @pygg/cli@dev` in `node:22-bookworm-slim`: `@pygg/linux-x64` found, `--version`, the same suggestion and exit 2). The `latest` tag of `@pygg/cli` and `@pygg/win32-x64` still points at `0.1.0-dev.0` (`npm dist-tag add @pygg/cli@0.1.0-dev.1 latest` moves it). The launcher and the four platform packages carry this version. It adds `pygg setup claude|codex|opencode` and `pygg run claude|codex|opencode` (a routing session per launch), a bare `pygg` that shows where you are, the read commands `providers`, `models`, `logs` and `usage` with `--json` on them and on `status` and `doctor`, strict options with exit codes 0, 1, 2 and 3, `pygg mcp setup` for the experimental MCP registration (which used to be `setup codex`), and the launcher's own errors and `--launcher-info` (docs/design/CLI.md, Codex's review of 2026-11-14). Before publishing: a release build, `node scripts/npm-pack.mjs`, then from an empty folder `npm install -g @pygg/cli@dev` and check `pygg`, `pygg --version`, `pygg doctor`, `pygg setup`, `pygg providers`, `pygg logs --json`, `pygg run claude --help` (Claude Code's own help), in Windows PowerShell 5.1, PowerShell 7 and cmd. Release note: "Set up and start Claude Code, Codex and OpenCode through Pygg; see providers, models, requests and cost from the terminal."

- `@pygg/win32-x64@0.1.0-dev.0`, 2026-11-12 (tags `dev` and `latest`, since it was the package's first version).
- `@pygg/cli@0.1.0-dev.0`, 2026-11-12 (tag `dev`). Installed back from the registry into an empty folder: npm pulled `@pygg/win32-x64` with it, `pygg doctor` found the running app (schema v19) and `pygg mcp` answered `initialize` and `tools/list`.
