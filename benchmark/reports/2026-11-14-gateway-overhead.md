# Gateway overhead, 2026-11-14

**Question.** How much time does Pygg's gateway add to a request? The production readiness review listed "no measured
overhead" as a reason performance was not ready for a claim.

**Answer.** About **4 ms** for a short request and **5 ms** for a 400 kB prompt (about 100 000 tokens), at the median,
on one developer PC. Most of it is the ledger: every attempt is written to disk *before* it is sent and settled after it
returns, each time with a full disk flush. A coding agent's request takes 1 to 60 seconds at the provider, so the
gateway adds well under 1 % to it. With 16 clients sending at once, the gateway handles about **230 requests a second**,
because the ledger writes take turns; that is far more than a person's coding tools send, but it is the limit.

## Method

`gateway::tests::overhead` (`apps/desktop/src-tauri/src/gateway/tests.rs`), an ignored test run in release:

```text
cargo test --release --lib gateway::tests::overhead -- --ignored --nocapture
```

- A fake OpenAI-compatible provider on a loopback port answers at once (no model, no network), so the difference between
  the two sides is the gateway's own cost.
- The same requests go to the fake directly and through a real gateway on a **store file** (SQLite in WAL mode, the
  app's settings): token check, routing, the client's limits, the attempt journaled before sending, the answer relayed,
  usage priced and settled in one transaction, the session pin and health updated.
- Four sequential scenarios, 20 warm-up requests then 400 (short) or 150 (400 kB) measured each side: a short prompt and
  a 400 000-character prompt, each whole and streamed. Then 16 concurrent clients sending 125 short requests each.
- Latency is from sending the request to having read the whole answer.
- Three runs. Raw figures: `benchmark/results/2026-11-14-gateway-overhead/run-{1,2,3}.json`, with `SHA256SUMS`.

Machine: Intel Core i7-8700K, 32 GB, Windows 11 Pro; the store on a Samsung PM981 NVMe SSD (the temp folder on C:).

## Results

Time added at the median (gateway p50 minus direct p50), milliseconds:

| Scenario | Run 1 | Run 2 | Run 3 |
| --- | ---: | ---: | ---: |
| Short prompt | 3.75 | 3.75 | 3.79 |
| Short prompt, streamed | 3.91 | 3.87 | 3.91 |
| 400 kB prompt | 5.40 | 5.11 | 4.95 |
| 400 kB prompt, streamed | 5.15 | 5.33 | 5.11 |

Run 3 in full:

| Scenario | Direct p50 | Direct p95 | Gateway p50 | Gateway p95 | Gateway p99 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Short prompt | 0.15 | 0.18 | 3.94 | 5.56 | 9.53 |
| Short prompt, streamed | 0.15 | 0.22 | 4.07 | 5.30 | 9.74 |
| 400 kB prompt | 1.78 | 2.64 | 6.73 | 12.59 | 29.15 |
| 400 kB prompt, streamed | 1.57 | 2.38 | 6.67 | 9.42 | 30.21 |

16 clients at once, short prompts:

| | Requests per second | p50 | p95 | p99 |
| --- | ---: | ---: | ---: | ---: |
| Direct (runs 1–3) | 24 575 / 24 007 / 25 593 | 0.6 ms | 0.9 ms | 1.2 ms |
| Through the gateway (runs 1–3) | 231 / 232 / 233 | 67 ms | 93 ms | 104–110 ms |

Every one of the 3 180 requests of a run left exactly one ledger entry.

## Where the time goes

A one-off run with SQLite's `synchronous = NORMAL` (the WAL is flushed at checkpoints, not at each commit;
`PYGG_OVERHEAD_SYNC=NORMAL`) showed how much of it is the disk flush:

| | Default (`FULL`) | `NORMAL` |
| --- | ---: | ---: |
| Added p50, short prompt | 3.8 ms | 1.1 ms |
| Added p50, 400 kB prompt | 5.1 ms | 2.6 ms |
| 16 at once | 232 requests/s | 565 requests/s |

So about 2.7 ms of the 3.8 ms is waiting for the disk to confirm the two ledger transactions. **Pygg keeps `FULL`.** The
attempt journal exists so that an attempt that reached a provider is counted even if Pygg, Windows or the power stops
before its answer is recorded (counter-review CR-06, the fault tests in `faults.rs`); with `NORMAL`, a power cut could
lose the journal entry of a request the provider billed. A few milliseconds on requests that take seconds is the better
trade for a tool that counts money.

The rest (about 1 ms, and the 565 requests/s ceiling) is the work done under the store's single connection: limits,
journal, pricing, settlement, pins and health. Under concurrent load requests wait for it in turn.

## What this does not measure

- A real provider's network time and model latency, which dominate every real request; the overhead's share of a real
  request is the added time over that request's total.
- Translation between formats (an OpenAI-format client to an Anthropic connection): these runs pass OpenAI to OpenAI.
- Hours of traffic, memory over days, a ledger of millions of rows, slower disks (a hard disk's flush takes several
  milliseconds more), or another computer.

## Later

- Settle the ledger in batches under load (one transaction for several answers), keeping the journal entry before each
  send: it would lift the concurrent ceiling without giving up the journal's guarantee.
- The same measurement with translation, and on a hard disk.
