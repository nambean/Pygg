# Contributing to Pygg

Pygg is MIT-licensed ([LICENSE](LICENSE)). Start with [docs/README.md](docs/README.md), which lists every document and the rules they follow, and [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Before you change code

- **Order comes from [ROADMAP](docs/ROADMAP.md).** Work that is not there, or is listed as withdrawn, needs a decision first ([DECISIONS](docs/DECISIONS.md)).
- **Status lives in one place.** Only [Implementation Status](docs/IMPLEMENTATION-STATUS.md) says what is built and how it was verified; other documents link to it instead of repeating it (review RM-1). A change that alters a line there updates it in the same commit.
- **Respect the decisions that bound the product:** official agent CLIs with their own sign-in and no session, cookie or OAuth bridges (ADR-026); tasks start from the IDE, the app monitors and approves, and nothing approves a paid step but the user (ADR-027, ADR-010); no chat in the app (ADR-028); the gateway never rewrites an external client's conversation (ADR-025).

## Building and testing

```powershell
pnpm install
pnpm typecheck
pnpm build
cd apps/desktop/src-tauri
cargo test
python ../../../scripts/schema-reference.py --check
```

- Every change to the core comes with tests. Gateway behaviour is tested end to end against the fake provider in `gateway/tests.rs`; a provider's real error shape goes in `tests/fixtures/provider-errors` (captured with `scripts/capture-provider-errors.ps1`, which never saves a body that contains a key).
- **Schema changes** are a new numbered migration in `migrations/`; an existing migration is never edited. Regenerate [docs/SCHEMA.md](docs/SCHEMA.md) with `python scripts/schema-reference.py`.
- **Interface text** is English, used as the key, with its Vietnamese translation in `src/locales/vi`. Screens are reviewed with the `apple-design` skill.

## Live tests and money

- **Never commit a key or token.** Keys come from environment variables; test keys put in the OS credential store are removed afterwards.
- A live run against a paid provider needs the owner's approval and fits the benchmark budget (ADR-028). Record what it verified, the model, the date and what it cost in Implementation Status.
- Say what was not verified. "Built" names its code; "verified" says how.

## Writing

Follow the writing rules in [docs/README.md](docs/README.md#writing-rules): Pygg as the product's name, absolute dates, claims with their evidence, unknown costs never shown as $0.

## Commits

Small commits with a subject that says what changed and why (`feat(gateway): …`, `fix(bench): …`, `docs: …`). Reference the ROADMAP item or review finding the change answers.
