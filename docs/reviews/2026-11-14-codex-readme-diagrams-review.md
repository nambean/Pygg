# Codex on the README's animated diagrams, 2026-11-14

> **Status:** Record · Codex CLI (`codex exec --sandbox read-only --ephemeral`, the owner's ChatGPT plan) reviewed the first render of [`scripts/readme-diagrams.mjs`](../../scripts/readme-diagrams.mjs) from nine PNG frames (light, dark and reduced-motion, rendered with headless Edge) and the repository · its answer is quoted below as written, then what was changed

## What was asked

Critique two animated SVGs for the README, shown on GitHub through `<picture>` and `prefers-color-scheme`: `usage-{light,dark}.svg` (a terminal using `pygg`, then the gateway hub) and `request-flow-{light,dark}.svg` (one request through the gateway: route, policy and budget, journal, key ring, a 429 and a 402, the next target, the ledger). Five questions: composition, legibility at README width, motion, accuracy against the code, and robustness in GitHub's image renderer.

## Codex's answer

Both compositions work, but the request-flow image needs less small copy and a longer final hold. Prioritized changes:

1. **Fix the usage sequence.** `setup codex` followed by `run claude` looks like one complete onboarding path, but Claude needs its own setup. Also, `run` checks an already-running gateway; it does not start it. Use `setup codex → run codex`, and state “Connect a provider and start the gateway first.” Evidence: [run.rs:75](../../apps/desktop/src-tauri/src/cli/run.rs#L75), [run.rs:90](../../apps/desktop/src-tauri/src/cli/run.rs#L90). The npm version/platform caption matches the package manifest.

2. **Qualify the failure footer.** Replace “A 5xx or a timeout stops…” with “By default, server errors stop; ambiguous timeouts always stop.” Some 5xx responses permit fallback when the client explicitly allows retries; connection failures can also fall back. Evidence: [mod.rs:1093](../../apps/desktop/src-tauri/src/gateway/mod.rs#L1093), [mod.rs:1148](../../apps/desktop/src-tauri/src/gateway/mod.rs#L1148).

3. **Clarify what the technical example represents.**
   - Label it “Example combo route.” `auto` does not universally prefer API keys before local models; ranking considers tier, then billing and estimated cost: [auto.rs:13](../../apps/desktop/src-tauri/src/gateway/auto.rs#L13).
   - Change “spend held up front” to “budget checked; estimated spend reserved when applicable.” Free, subscription and unlimited clients reserve zero: [mod.rs:911](../../apps/desktop/src-tauri/src/gateway/mod.rs#L911).
   - Add “text only; no tools” to web sessions. The current coding-agent framing overpromises their applicability: [websession/mod.rs:44](../../apps/desktop/src-tauri/src/websession/mod.rs#L44).
   
   The remaining core claims check out: loopback/client authentication, per-attempt journaling before sending, separate-account key rings, 429/402 cooldowns, next-target fallback, streaming and settlement, subscription quota accounting, and unknown cost remaining unknown. “No money” should become “no provider API charge.”

4. **Enlarge explanatory text by cutting it.** At 830 px, 10.5–12.5 px source text becomes approximately **9.3–11 px**. The flow footer, legend, return-path sentence and ledger notes are the weakest reading targets. Move policy caveats into adjacent README prose; keep diagram labels around 14 px source size. Preserve the five-step top row, but reduce its descriptions. Usage’s two-panel hierarchy is clearer; trim the version comment and bottom security caption. No obvious overlap or clipping appears in the supplied frames.

5. **Increase secondary-label contrast.** Light-theme faint text is approximately **3.04:1 on white**; dark faint text is **3.77:1 on the panel**—insufficient for these small labels. Darken/lighten `.cap` and `.h` toward `.muted`. Ordinary dark-theme secondary text is considerably better, approximately 5.94:1. Keep inactive hub labels readable instead of dimming the entire hub to 35%.

6. **Give the story’s conclusion time.** The flow’s settled third row appears only around **10.99–11.4 seconds: roughly 0.4 seconds**. Hold the completed ledger for 2–3 seconds. Keep 429/402 badges visible after refusal. In usage, remove the continuously bouncing signal bars and reduce halo motion; four simultaneous outgoing pulses suggest broadcast rather than selection.

7. **Harden static and browser behavior.** The supplied reduced-motion usage frame shows a cursor over the command and an overpowering halo. Current checked-in CSS already hides cursor groups, so reconcile the capture with current output; explicitly give the static halo low opacity. Make the complete diagram the base state, then apply animation overrides, so suppressed animation remains informative.

   Self-contained CSS animation is compatible with SVG image processing; scripts/external resources are restricted ([SVG specification](https://www.w3.org/TR/svg-integration/)). `steps()` and `transform-box` are not obvious blockers. Font-dependent typing widths are the greater portability risk. Test actual GitHub `<img>` rendering in Safari and Firefox; Edge frames alone do not establish compatibility.
## What was changed

| # | Finding | Done |
| --- | --- | --- |
| 1 | `pygg run` does not start the gateway, and Claude Code needs `pygg setup claude` first | The terminal now runs `npm install`, `pygg status` (the gateway answers; the hub lights up then), `pygg setup claude` ("its own Pygg key; no file changes", as `cli/setup.rs` says), `pygg run claude`. Requests flow only after the last command. |
| 2 | 5xx stops only by default; a timeout always does | Footer: "A timeout stops the request, and so does a 5xx unless the client allows retries." |
| 3 | The flow is one example, not what `auto` always does; spend is held only for a capped key; web sessions are text only; local is not "no money" | Title "an example route"; Policy "spend held if capped"; Ollama "no API charge"; web sessions "text only". |
| 4 | Too much small copy | Usage: the status block, the chips, the signal bars and the bottom caption removed; flow: shorter box text. Policy caveats moved into the README prose. |
| 5 | Low contrast of captions and headers | Captions and table headers use the muted text colour instead of the faint one; the hub dims to 45%, not 35%. |
| 6 | The settled ledger showed for 0.4 s; four pulses at once read as a broadcast | The story takes the first 74% of a 15 s loop and the finished ledger holds about 3.5 s; the 429 and 402 badges stay. In the usage image one target is lit at a time; the halo no longer scales. |
| 7 | Reduced-motion frame showed a cursor and a heavy halo | Every cursor hidden, the halo fixed at 20% opacity, all four typing covers removed in the static state (re-rendered and checked). |

Not done: a test in Safari and Firefox through GitHub's own `<img>` (no such browsers on this PC); the SVGs use only CSS keyframes on `opacity`, `transform: translate`, `stroke`, `fill` and `stroke-dashoffset`, and no `offset-path`, script or external resource.
