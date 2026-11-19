# Contributing to Pygg

> **Status:** Current · how to change Pygg: order, documents, tests, schema, money, writing, commits · reviewed 2026-11-14

Pygg is MIT-licensed ([LICENSE](LICENSE)). Start with [docs/README.md](docs/README.md), which lists every document, and [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Before you change code

- **Order comes from the [ROADMAP](docs/ROADMAP.md).** Work that is not there, or is listed as withdrawn, needs a decision first ([DECISIONS](docs/DECISIONS.md)).
- **Status lives in one place.** Only [IMPLEMENTATION-STATUS](docs/IMPLEMENTATION-STATUS.md) says what is built and how it was verified; other documents link to it instead of repeating it. A change that alters a line there updates it in the same commit.
- **Respect the decisions that bound the product:** official agent CLIs with their own sign-in, and no session, cookie or OAuth bridges (ADR-026); tasks start from the IDE, the app monitors and approves, and nothing but the user approves a paid step (ADR-027, ADR-010); no chat in the app (ADR-028); the gateway never rewrites an external client's conversation (ADR-025).

## Documents

`docs/` holds final documents only. Each one describes Pygg as it is now, for its reader.

- **Change the document in place** to the new truth when the code changes. Do not append a dated note, an "Update" paragraph or a history of how it got there.
- **No dated logs, review records or progress reports in `docs/`.** Reviews, answers to them and progress notes belong in issues, pull requests and commit messages. Benchmark results are the exception: each run is a report in `benchmark/reports/`, with its raw results in `benchmark/results/`.
- **Delete what is superseded.** Do not move it to an archive; Git keeps the history. Update or remove every link to it.
- **One status line** under each title: `> **Status:** Current · <what it is> · reviewed <date>`.
- **User documents** (GETTING-STARTED, PROVIDERS, SECURITY, BENCHMARKS) say only what IMPLEMENTATION-STATUS says is built, and say "Not verified yet" where a reader would otherwise assume something works.

## Building and testing

```powershell
pnpm install
pnpm typecheck
pnpm build                       # the desktop crate embeds the built interface
cd apps/desktop/src-tauri
cargo fmt --check                # cargo fmt formats as rustfmt.toml says
cargo clippy --all-targets --all-features -- -D warnings
cargo clippy --no-default-features --all-targets -- -D warnings
cargo test --all-features --lib --tests   # with the integration tests in tests/
cd ../../..
python scripts/schema-reference.py --check
python scripts/licences.py --check
python scripts/model-facts.py selftest   # after a change to the model facts builder (pip install pyyaml pyarrow cryptography)
```

CI runs all of these on Windows, and the layout check below. A separate workflow runs `cargo audit` and `pnpm audit` when a lockfile changes and every Monday; a known vulnerability fails it. CodeQL scans the Rust, TypeScript, Python and workflow code on every push and weekly. Actions in the workflows are pinned to commit SHAs; Dependabot proposes updates monthly.

- **No lint warnings.** Clippy runs with `-D warnings` for the app (`--all-features`) and for the console program (`--no-default-features`). Three lints are allowed on purpose, with the reason, at the top of `apps/desktop/src-tauri/src/lib.rs`; do not add more without a reason there.
- **Every change to the core comes with tests.** Gateway behaviour is tested end to end against the fake provider in `gateway/tests.rs`. A provider's real error shape goes in `tests/fixtures/provider-errors`, captured with `scripts/capture-provider-errors.ps1`, which never saves a body that contains a key.
- **Schema changes** are a new numbered migration in `apps/desktop/src-tauri/migrations/`; an existing migration is never edited. Regenerate [docs/SCHEMA.md](docs/SCHEMA.md) with `python scripts/schema-reference.py`.
- **Licences.** A new Rust crate or interface package must have an allowed licence. Regenerate the inventory with `python scripts/licences.py`. Code or data taken from another project goes in [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md) with its notice.

## Interface

- **Run `pnpm ui-check` before calling interface work done.** Start the browser preview first (`pnpm dev`, port 1431). The check opens it with `?stress` data (long, real-shaped names and errors) in headless Edge or Chrome at five sizes: 1024×640 (the app's minimum), 1440×900, 1990×1030, and 819×512 and 683×427 (the 1024 px window at Windows text size 125% and 150%). It visits every screen and a set of clicked states (sheets, menus, expanded rows), and fails when content is wider than its container, reaches past the window's edge, or is squeezed to one word per line. `--sizes WxH,…` picks other sizes. It plants a known layout bug as a self-test and exits 2 if it misses it.
- **Never put variable-length text in a `nowrap` cell or beside buttons.** Results, errors, names and model ids vary in length; give them a line of their own that can wrap.
- **Interface text** is English and goes through `t()` (`src/lib/i18n.ts`), with the English text as the key. The Vietnamese translations in `src/locales/vi` are added at the end, not with each change.

## Live tests and money

- **Never commit a key or token.** Keys come from environment variables; test keys put in the OS credential store are removed afterwards.
- **A live run against a paid provider** needs the maintainer's approval and fits the benchmark budget ($20 in total, ADR-028). Record what it verified, the model, the date and what it cost in IMPLEMENTATION-STATUS.
- **Web sessions** are real accounts on sites that may ban them. Live checks are short questions only, with the guard on; never point a benchmark of long prompts at a web session.
- **Say what was not verified.** "Built" names its code; "verified" says how (unit test, fake provider, live run).

## Writing

- **Names.** The product is **Pygg** everywhere: prose, the repository, packages, crates, `x-pygg-*` headers and `PYGG_*` variables. The engineering name SmartTaskRouter stays in two identifiers only, so existing data stays readable: the app identifier `app.smarttaskrouter.desktop` (it names the folder that holds the user's store) and the credential service name `SmartTaskRouter`. The gateway still reads the old session header `x-smarttaskrouter-session`, which clients set up before the rename send.
- **Plain English**, short sentences, and absolute dates (2026-11-14) where a date is needed.
- **Claims carry their evidence**, and untested is said plainly.
- **Costs are honest.** Unknown is never $0; an estimate says what it was estimated from; a subscription's use is quota, not money.
- **Decisions** on licence, budget and scope are recorded as ADRs in [DECISIONS](docs/DECISIONS.md) and cited by number.

## Commits

Small commits whose subject says what changed and why (`feat(gateway): …`, `fix(bench): …`, `docs: …`). Name the ROADMAP item the change answers, if there is one.
