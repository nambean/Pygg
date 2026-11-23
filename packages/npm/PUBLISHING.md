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

1. Raise `version` in `packages/npm/pygg/package.json` and the four `optionalDependencies` entries together (`npm-pack.mjs` refuses versions that differ). The platform packages take the launcher's version. `pygg --version` reports it too: the build reads it from this package.json, and `npm-pack.mjs` refuses a program built before the change. Also the pinned `@pygg/cli@<version>` in `plugins/pygg/scripts/mcp.mjs` (npm-pack refuses another) and the plugin's `version` in `plugins/pygg/.claude-plugin/plugin.json`: Anthropic's plugin directory picks up each commit to `main`.
2. Push to `main` (or run **npm packages** from the Actions tab). The workflow builds `pygg-cli` on Windows, Linux and macOS, packs every package, installs each platform's back from the tarballs and runs `pygg`. Download the artifact **npm-packages-all** (five `.tgz` files) from the run.
3. Publish, from the folder holding the tarballs, platform packages first:

```powershell
npm publish pygg-win32-x64-0.1.0-dev.2.tgz --access public --tag dev
npm publish pygg-darwin-arm64-0.1.0-dev.2.tgz --access public --tag dev
npm publish pygg-darwin-x64-0.1.0-dev.2.tgz --access public --tag dev
npm publish pygg-linux-x64-0.1.0-dev.2.tgz --access public --tag dev
npm publish pygg-cli-0.1.0-dev.2.tgz --access public --tag dev
```

`--tag dev` keeps a pre-release off `latest`, and current npm refuses a pre-release version without a tag. npm asks for the two-factor code at each publish. People install it with `npm install -g @pygg/cli@dev`, then run `pygg`.

4. Check from a clean folder on each system you have: `npm install -g @pygg/cli@dev`, `pygg --launcher-info` (names the platform package it runs), `pygg --version`, `pygg doctor`.

5. **The MCP Registry** (once the new version is on npm). `server.json` names the package at the launcher's version, and npm-pack refuses a version that differs or a `mcpName` that is not its `name`; the registry checks that the published package carries that `mcpName`, so a version published before 2026-11-20 (`0.1.0-dev.7` and older) cannot be listed. From `packages/npm/pygg`, with `mcp-publisher` from the [registry's releases](https://github.com/modelcontextprotocol/registry/releases):

```powershell
mcp-publisher login github     # as nambean: the name io.github.nambean/pygg needs that account
mcp-publisher publish
```

   Check: `https://registry.modelcontextprotocol.io/v0.1/servers?search=io.github.nambean/pygg`.

A Windows-only package can still be made locally after a release build: `node scripts/npm-pack.mjs` packs this computer's platform package and the launcher into `dist/npm`. Publishing that alone leaves the other platforms' optional dependencies missing, which the launcher reports on those systems.

## Published

| Version | Date |
| --- | --- |
| `0.1.0-dev.9` | 2026-11-23 |
| `0.1.0-dev.8` | 2026-11-20 |
| `0.1.0-dev.7` | 2026-11-19 |
| `0.1.0-dev.6` | 2026-11-18 |
| `0.1.0-dev.5` | 2026-11-18 |
| `0.1.0-dev.4` | 2026-11-18 |
| `0.1.0-dev.3` | 2026-11-18 |
| `0.1.0-dev.2` | 2026-11-16 |
| `0.1.0-dev.1` | 2026-11-14 |
| `0.1.0-dev.0` | 2026-11-12 |
