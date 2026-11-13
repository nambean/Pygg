# Third interface review by Codex: the screens, the savings story and the npm path, 2026-11-13

> **Status:** Record · Codex's review as it wrote it (read-only, no files changed), and what was done · earlier rounds: [first](2026-11-12-codex-ux-review.md), [second](2026-11-12-codex-review-round2.md) · order: [ROADMAP](../ROADMAP.md) · built: [Implementation Status](../IMPLEMENTATION-STATUS.md)

**How it was run.** Codex CLI 0.163.0 (ChatGPT plan), `codex exec --sandbox read-only --ephemeral`, given ten screenshots of the browser preview (English, dark, demo data and the first-run state; the preview now opens on a screen with `?screen=usage`) and the source. It read both reference READMEs, 9router's overview-card source and OmniRoute's CLI documentation; OmniRoute's Home and analytics source it could not fetch. The owner's three questions that day: the screens against 9router and OmniRoute, why Home and Usage do not yet show *how* Pygg saves, and how to make the npm path easiest. Every finding was checked against the code before it was acted on.

## A. Screens against 9router and OmniRoute

Codex: keep the reversible setup, the per-tool request verification, the separate API URLs and the cost provenance.

| # | Finding (Codex) | Answer |
| --- | --- | --- |
| 1 | Home can complete onboarding after a failed first request; a connected but ineligible provider still counts as done | **Done.** The last step is "First request answered" and completes only when a tool's newest request was answered (`latestRequests`, outcome OK). Step one stays open while auto has no eligible model, with the existing explanation |
| 2 | CLI Tools explains implementation before telling people what to do; the empty state repeats the provider warning three times | **Done.** One sentence per tool, with "What Set Up changes" as a disclosure under Claude Code. With no provider at all, one warning above the list ("Connect a provider before sending requests through Pygg…") and none under the rows |
| 3 | Providers mixes health, billing knowledge and routing readiness without a next action | **Later.** A compact "auto readiness" row (how many models qualify, how many lack routing facts) is the next Providers change; not in this round |
| 4 | Usage makes investigation a long scroll; failed tool status only mentions Usage in text | **Later** (a Summary / Requests split, and a filtered link from CLI Tools). The request list, its filters and Show More stay as built in round two |
| 5 | Usage's empty state gives no action and sits above Task Results | **Done.** "No gateway requests in this period", one sentence on what to do, and Open CLI Tools |
| 6 | Endpoint's prominent Copy URL copies the root, although Codex and editors need `/v1` | **Done.** The gateway card on Endpoint has Copy OpenAI URL and Copy Anthropic URL; Home keeps one Copy URL |
| 7 | Combos does not say whether one is needed | **Done.** Above the list: "Using auto? You don't need a combo…", with the fallback rule in one clause. The tab keeps its name, How auto chooses, which Home and CLI Tools point to |
| 8 | Guide's quick start omits the API key from its recipe, treats Ollama like a key provider, and hardcodes the port | **Done.** Cloud provider versus local runtime in step one; the key line in the recipe; the addresses come from the running gateway's status |
| 9 | Home overgeneralizes success from one client | **Done.** "Latest answered request: {client} → {model} · {time}. Each tool's own state is in CLI Tools." The 24-hour stat reads "Fallback attempts · another model was tried" |

## B. The savings story

Codex: the user-chosen baseline, the coverage count, the exclusions and the same-token caveat are more defensible than the references' savings promises; keep the estimate label, as 9router's "Est. Cost ~$X · Estimated, not actual billing" does. Its proposed order: readiness → alert → comparison → billing mix → trend → by client → routing activity → cost breakdowns → requests.

| # | Finding (Codex) | Answer |
| --- | --- | --- |
| 1 | The hero has the right figures but makes the counterfactual hard to scan | **Done.** Under the amount: "{n} of {total} answered requests compared with {model}", then three aligned figures: Priced at {model}, Cost through Pygg, Estimated difference (or Estimated extra cost when negative) |
| 2 | The billing mix is returned but never rendered | **Done.** "Where the answers came from": one bar with Free, Local, Subscription, Paid and Billing unknown, each with its count and share of all answered requests, shown with or without a baseline. The line above it counts the free, local and plan answers; it does not call them money saved |
| 3 | The qualifications are honest but buried in dense prose | **Done.** Exclusions in one short line; failed attempts of unknown cost in their own; the method in a disclosure, "How this estimate works", which also says that billing and prices come from the current settings and that an answer from a free, local or plan model is not proof of avoided spending |
| 4 | The client table shows spend, not benefit | **Done.** Usage → How Pygg Saved It → By client: compared / answered, cost through Pygg, the same tokens at the baseline, the difference; a client opens its requests. Backend: `savings.rs` returns the estimate by client (`by_client`) |
| 5 | The trend shows spending alone | **Done.** The usage chart has a fourth metric, Comparison: the same tokens at the baseline behind, what they cost through Pygg in front, per hour or day, with the difference and the compared count in the tooltip. Backend: `periods` from the same accumulator, aligned as the usage chart's buckets |
| 6 | Routing evidence is hidden inside request details | **Done** by how the model was asked for: auto, auto/cheap, a combo (Pygg's choice) and a model by name (the user's choice, "listed, not claimed"). Backend: `by_route` from `requested_model`, never from the human-readable reason. The finer split Codex drew (auto by billing, cheapest-first against ordered combos) needs a stored routing code per event; later |
| 7 | Fallback counts cannot establish that requests were rescued | **Done** as Codex asked for now: "{n} fallback attempts… a fallback keeps a request alive; it is not in itself a saving." A logical request id across attempts, and so "answered after fallback", is a later backend slice |
| 8 | The comparison cohort and total spending are different measures | **Done.** The method disclosure reconciles: the compared answers' cost plus failed attempts with a known cost; "failed attempts with a known cost are in the total above, not in these rows" under the tables |
| 9 | Historical billing follows current settings; disconnected attempts disappear | **Done in part.** The disclosure says billing and prices follow the current settings. A client that left is no longer skipped: a known charge counts in the failed-attempt cost, an unknown one in the count. Recording the billing on each event is later |
| 10 | A routing choice is not proof of causal savings or equal-quality work | **Done.** "Estimated difference", never "you would have paid"; the mechanism lines say "used what you already have"; repeated tool output stays measured tokens, never money |

## C. The npm path

Codex: the launcher's execution is sound (arguments as an array, inherited streams, the native exit code); the defects are recovery, version visibility and release drift.

| # | Finding (Codex) | Answer |
| --- | --- | --- |
| 1 | The documented commands exceed the published binary | **Prepared, not published.** Both packages carry `0.1.0-dev.1`; `PUBLISHING.md` lists what to check from an empty folder before `npm publish`. Publishing is the owner's step |
| 2 | First run starts with diagnostics but does not bridge to a running gateway and provider | **Done.** README, Guide and the bare command lead with: open Pygg and connect a provider, then `pygg doctor` → `pygg setup claude-launcher` → `pygg run claude`; Codex and OpenCode in one line each; "npm installs the command only" |
| 3 | Bare `pygg` prints a long help page to stderr and exits 2 | **Done.** Five lines to stdout, exit 0: where to start and the three commands. `--help` stays the full list; an unknown command still exits 2 |
| 4 | Package, executable and preview channel need one consistent explanation | **Done.** "Install `@pygg/cli`; run `pygg`. `@dev` is the Windows x64 developer preview." in the README and the Guide |
| 5 | No try-without-global-install recipe | **Done.** `npx --yes --package=@pygg/cli@dev pygg doctor` (and setup, run), with what npx does and that setup still changes the tool's configuration |
| 6 | Missing-binary errors omit the repair and overpromise the app fallback on other platforms | **Done.** On Windows x64 the launcher lists where it looked and the three repairs; on other platforms it says no native program is published. Errors go to stderr only, so `pygg mcp` keeps stdout |
| 7 | Launcher precedence can hide why updated commands are missing | **Done.** `PYGG_CLI` must be a full path and must exist; `pygg --launcher-info` prints the launcher version, every candidate with found / not installed / not there, and which one runs. Nothing is printed during normal or MCP runs |
| 8 | Windows troubleshooting stops at "open a new terminal" | **Done.** "If pygg is not found": `where.exe pygg`, `npm prefix -g`, the PATH step, `pygg.cmd` for a PowerShell script policy, the SmartScreen note |
| 9 | Help and README foreground task/MCP setup, inviting the wrong command | **Done.** `--help` in three groups: Use coding tools through Pygg, Manage the gateway, Experimental task integration (on hold). The Guide's command table says which `setup` names route requests and which register the MCP server |

**Left for later:** Providers' readiness row (A3), the Summary / Requests split and the filtered link from CLI Tools (A4), a stored routing code and billing per event with a logical request id across attempts (B6, B7, B9), and the publish itself (C1).
