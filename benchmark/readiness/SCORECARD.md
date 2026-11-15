# Readiness scorecard

Run 2026-11-15T1128Z on commit `5cd660b` (with uncommitted changes), 607 s. Free and local traffic only. Made by `python -I benchmark/readiness/run.py`; what each gate means is in [docs/ROADMAP.md](../../docs/ROADMAP.md#release-gates).

| Gate | State | Checks passed | Still needed |
| --- | --- | --- | --- |
| Parity | **open** | 11 of 11 | live parity per claimed cloud provider path (two turns, caching, tools, streaming): only the fake provider and local Ollama are measured here |
| Honesty | **green** | 3 of 3 | - |
| Security | **green** | 4 of 4 | - |
| Errors | **open** | 4 of 4 | real provider answers not yet captured as fixtures: 403, 429, quota, timeout (13 fixtures today) |
| Install | **open** | 6 of 6 | clean-machine NSIS install and uninstall; signed installer (release only) |
| Interface | **open** | 1 of 1 | every screen clicked through in the native window; D12 checks (scaling, snap layouts, Narrator and NVDA, contrast themes); Vietnamese strings 48.0% complete (699 of 1455 found in t() calls; translation is scheduled last) |
| Web sessions | **green** | 1 of 1 | - |
| Performance (tracked) | **green** | 1 of 1 | - |
| Routing (tracked) | **green** | 3 of 3 | - |

## Checks

| Gate | Check | Result | Numbers |
| --- | --- | --- | --- |
| Install | `release_build` | pass | seconds 1.2 |
| Parity | `openai_chat_whole` | pass | status 200, route readiness-a/fake-model |
| Parity | `openai_chat_stream` | pass | status 200, route readiness-a/fake-model |
| Parity | `anthropic_messages_whole` | pass | status 200 |
| Parity | `anthropic_messages_stream` | pass | status 200 |
| Parity | `responses_whole` | pass | status 200 |
| Parity | `tools_openai_whole` | pass | status 200, tool read_file |
| Parity | `tools_openai_stream` | pass | status 200, tool read_file |
| Parity | `tools_anthropic_whole` | pass | status 200 |
| Parity | `tools_anthropic_stream` | pass | status 200 |
| Parity | `anthropic_cache_parity` | pass | direct_cache_reads [0, 2407, 2407], pygg_cache_reads [0, 2407, 2407] |
| Security | `session_affinity_ring` | pass | keys_one_session 1, keys_no_session 3 |
| Security | `ring_key_needs_confirmation` | pass | exit 1 |
| Errors | `fallback_on_429` | pass | status 200, attempts 2, upstream http_429,ok |
| Errors | `fallback_on_401` | pass | status 200, attempts 2, upstream http_401,ok |
| Errors | `fallback_on_404` | pass | status 200, attempts 2, upstream http_404,ok |
| Honesty | `server_error_cost_unknown` | pass | client_status 500, ledger error/unknown/500 |
| Security | `strict_budget_concurrency` | pass | served 2, refused 6, sent_upstream 2, spent_micro_usd 56 |
| Security | `loopback_only` | pass | lan_addresses_tried 2, reached 0 |
| Honesty | `ledger_settled` | pass | upstream_requests 33, ledger_rows 33, unsettled 0 |
| Honesty | `kill_restart_ledger_whole` | pass | in_flight 6, journaled_before_kill 6, upstream_saw 6, interrupted_entries 6, restart_ms 707, after_restart 200 |
| Routing | `auto_cheap_decision` | pass | auto_cheap_pick fake-cheap, without_tools_pick fake-model, excluded_reason does not take tools |
| Routing | `auto_context_fit` | pass | ranked 2, pick claude-haiku-5-5 |
| Routing | `auto_cheap_request` | pass | status 200, route readiness-a/fake-cheap |
| Parity | `ollama_parity` | pass | prompt_tokens_direct 27, prompt_tokens_pygg 27, ledger_tokens 27, direct_s 1.48, pygg_s 1.32 |
| Install | `setup_codex` | pass | setup_s 0.3, remove_s 0.1, key_made 1, restored True, backups 3 |
| Install | `setup_opencode` | pass | setup_s 0.2, remove_s 0.3, key_made 1, restored True, backups 5 |
| Install | `setup_claude_launcher` | pass | setup_s 0.1, remove_s 0.1, key_made 1, files_written 0 |
| Install | `no_credentials_left` | pass | entries_left 0 |
| Performance | `gateway_overhead` | pass | added_p50_short_ms 3.88, added_p50_400kB_ms 6.01, rps_16_clients 212 |
| Errors | `provider_error_fixtures` | pass | passed 4, failed 0, seconds 16.3 |
| Web sessions | `web_guard_tests` | pass | passed 52, failed 0, seconds 197.6 |
| Interface | `ui_check` | pass | exit 0, findings 0, seconds 330.2 |
| Install | `npm_install_back` | pass | pack_s 4.4, install_s 1.6, packages 2, npm_version 0.1.0-dev.1, reports pygg-cli 0.1.0 |

Metrics: vi_strings_pct 48.0, error_fixtures 13.
