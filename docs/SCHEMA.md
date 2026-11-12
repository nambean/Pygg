# Store schema

> **Status:** Current · generated from the migrations by `scripts/schema-reference.py`; do not edit by hand · schema v20

The local SQLite store (`db.rs`): every table after all migrations, in order. Credentials are never here: a
connection holds only a reference to its key in the OS credential store (ADR-002). Client tokens are stored as
SHA-256 hashes.

## `approvals`

| Column | Type | Null | Default | Key | Added in |
| --- | --- | --- | --- | --- | --- |
| `id` | TEXT | yes |  | primary | 0001_init |
| `attempt_id` | TEXT | no |  |  | 0001_init |
| `cap_micro_usd` | INTEGER | no |  |  | 0001_init |
| `estimate_low_micro` | INTEGER | yes |  |  | 0001_init |
| `estimate_high_micro` | INTEGER | yes |  |  | 0001_init |
| `policy_version` | TEXT | no |  |  | 0001_init |
| `state` | TEXT | no |  |  | 0001_init |
| `created_at` | INTEGER | no |  |  | 0001_init |
| `decided_at` | INTEGER | yes |  |  | 0001_init |

References: `attempt_id` → `attempts.id` (on delete cascade).

1 CHECK constraint (see the migrations).

## `attempts`

| Column | Type | Null | Default | Key | Added in |
| --- | --- | --- | --- | --- | --- |
| `id` | TEXT | yes |  | primary | 0001_init |
| `stage_id` | TEXT | no |  |  | 0001_init |
| `number` | INTEGER | no |  |  | 0001_init |
| `agent` | TEXT | no |  |  | 0001_init |
| `model_ref` | TEXT | no |  |  | 0001_init |
| `tier` | INTEGER | no |  |  | 0001_init |
| `billing` | TEXT | no |  |  | 0001_init |
| `worktree_path` | TEXT | yes |  |  | 0001_init |
| `state` | TEXT | no |  |  | 0001_init |
| `check_results_json` | TEXT | yes |  |  | 0001_init |
| `escalation_reason` | TEXT | yes |  |  | 0001_init |
| `started_at` | INTEGER | yes |  |  | 0001_init |
| `finished_at` | INTEGER | yes |  |  | 0001_init |
| `rung` | INTEGER | no | `0` |  | 0008_task_engine |
| `base_revision` | TEXT | yes |  |  | 0008_task_engine |
| `agent_model` | TEXT | yes |  |  | 0008_task_engine |
| `detail` | TEXT | yes |  |  | 0008_task_engine |
| `files_changed` | INTEGER | yes |  |  | 0008_task_engine |
| `insertions` | INTEGER | yes |  |  | 0008_task_engine |
| `deletions` | INTEGER | yes |  |  | 0008_task_engine |
| `summary` | TEXT | yes |  |  | 0008_task_engine |
| `log_tail` | TEXT | yes |  |  | 0008_task_engine |
| `agent_estimate_micro_usd` | INTEGER | yes |  |  | 0010_approval_estimates |
| `handoff_sent` | TEXT | yes |  |  | 0017_task_handoff |
| `changed_paths_json` | TEXT | yes |  |  | 0017_task_handoff |
| `cost_source` | TEXT | yes |  |  | 0019_step_cost_source |

References: `stage_id` → `task_stages.id` (on delete cascade).

1 CHECK constraint (see the migrations).

## `audit_events`

| Column | Type | Null | Default | Key | Added in |
| --- | --- | --- | --- | --- | --- |
| `id` | TEXT | yes |  | primary | 0001_init |
| `actor` | TEXT | no |  |  | 0001_init |
| `action` | TEXT | no |  |  | 0001_init |
| `entity` | TEXT | no |  |  | 0001_init |
| `metadata_json` | TEXT | no | `'{}'` |  | 0001_init |
| `created_at` | INTEGER | no |  |  | 0001_init |

## `budget_reservations`

| Column | Type | Null | Default | Key | Added in |
| --- | --- | --- | --- | --- | --- |
| `id` | TEXT | yes |  | primary | 0003_cost_safe_gateway |
| `client_id` | TEXT | no |  |  | 0003_cost_safe_gateway |
| `reserved_micro` | INTEGER | no |  |  | 0003_cost_safe_gateway |
| `created_at` | INTEGER | no |  |  | 0003_cost_safe_gateway |
| `provider_id` | TEXT | yes |  |  | 0004_counter_review |
| `model_id` | TEXT | yes |  |  | 0004_counter_review |
| `requested_model` | TEXT | yes |  |  | 0004_counter_review |
| `session_key` | TEXT | yes |  |  | 0004_counter_review |
| `route_mode` | TEXT | yes |  |  | 0004_counter_review |
| `owner_generation` | TEXT | yes |  |  | 0006_owner_generation |

References: `client_id` → `gateway_clients.id` (on delete cascade).

Indexes: `reservations_by_client`.

1 CHECK constraint (see the migrations).

## `combo_members`

| Column | Type | Null | Default | Key | Added in |
| --- | --- | --- | --- | --- | --- |
| `combo_id` | TEXT | no |  | primary | 0002_gateway |
| `position` | INTEGER | no |  | primary | 0002_gateway |
| `provider_id` | TEXT | no |  |  | 0002_gateway |
| `model_id` | TEXT | no |  |  | 0002_gateway |

References: `provider_id` → `providers.id` (on delete cascade); `combo_id` → `combos.id` (on delete cascade).

## `combos`

| Column | Type | Null | Default | Key | Added in |
| --- | --- | --- | --- | --- | --- |
| `id` | TEXT | yes |  | primary | 0002_gateway |
| `name` | TEXT | no |  |  | 0002_gateway |
| `created_at` | INTEGER | no |  |  | 0002_gateway |

## `connection_health`

| Column | Type | Null | Default | Key | Added in |
| --- | --- | --- | --- | --- | --- |
| `provider_id` | TEXT | no |  | primary | 0003_cost_safe_gateway |
| `model_id` | TEXT | no | `''` | primary | 0003_cost_safe_gateway |
| `state` | TEXT | no |  |  | 0003_cost_safe_gateway |
| `reason` | TEXT | no |  |  | 0003_cost_safe_gateway |
| `http_status` | INTEGER | yes |  |  | 0003_cost_safe_gateway |
| `until_ms` | INTEGER | yes |  |  | 0003_cost_safe_gateway |
| `updated_at` | INTEGER | no |  |  | 0003_cost_safe_gateway |
| `strikes` | INTEGER | no | `1` |  | 0014_strict_budget_and_probes |

References: `provider_id` → `providers.id` (on delete cascade).

## `feedback`

| Column | Type | Null | Default | Key | Added in |
| --- | --- | --- | --- | --- | --- |
| `id` | TEXT | yes |  | primary | 0015_quality_signals |
| `metric_name` | TEXT | no |  |  | 0015_quality_signals |
| `target_kind` | TEXT | no |  |  | 0015_quality_signals |
| `target_id` | TEXT | no |  |  | 0015_quality_signals |
| `value_bool` | INTEGER | yes |  |  | 0015_quality_signals |
| `value_float` | REAL | yes |  |  | 0015_quality_signals |
| `source` | TEXT | no |  |  | 0015_quality_signals |
| `tags_json` | TEXT | yes |  |  | 0015_quality_signals |
| `created_at` | INTEGER | no |  |  | 0015_quality_signals |

References: `metric_name` → `metrics.name`.

Indexes: `feedback_by_metric`, `feedback_by_target`.

4 CHECK constraints (see the migrations).

## `gateway_clients`

| Column | Type | Null | Default | Key | Added in |
| --- | --- | --- | --- | --- | --- |
| `id` | TEXT | yes |  | primary | 0002_gateway |
| `name` | TEXT | no |  |  | 0002_gateway |
| `token_hash` | TEXT | no |  |  | 0002_gateway |
| `token_prefix` | TEXT | no |  |  | 0002_gateway |
| `monthly_budget_micro_usd` | INTEGER | yes |  |  | 0002_gateway |
| `created_at` | INTEGER | no |  |  | 0002_gateway |
| `last_used_at` | INTEGER | yes |  |  | 0002_gateway |
| `revoked_at` | INTEGER | yes |  |  | 0002_gateway |
| `max_concurrency` | INTEGER | no | `8` |  | 0003_cost_safe_gateway |
| `unknown_cost_policy` | TEXT | no | `'block'` |  | 0003_cost_safe_gateway |
| `allowed_connections` | TEXT | yes |  |  | 0005_client_scope |
| `paid_retries` | INTEGER | no | `0` |  | 0005_client_scope |
| `auto_min_tier` | TEXT | no | `'T2'` |  | 0007_auto_routing |
| `may_run_tasks` | INTEGER | no | `0` |  | 0009_tasks_from_clients |
| `require_no_training` | INTEGER | no | `0` |  | 0013_data_use_terms |
| `strict_budget` | INTEGER | no | `0` |  | 0014_strict_budget_and_probes |

8 CHECK constraints (see the migrations).

## `metrics`

| Column | Type | Null | Default | Key | Added in |
| --- | --- | --- | --- | --- | --- |
| `name` | TEXT | yes |  | primary | 0015_quality_signals |
| `type` | TEXT | no |  |  | 0015_quality_signals |
| `level` | TEXT | no |  |  | 0015_quality_signals |
| `optimize` | TEXT | no |  |  | 0015_quality_signals |
| `description` | TEXT | no |  |  | 0015_quality_signals |

3 CHECK constraints (see the migrations).

## `models`

| Column | Type | Null | Default | Key | Added in |
| --- | --- | --- | --- | --- | --- |
| `id` | TEXT | yes |  | primary | 0001_init |
| `provider_id` | TEXT | no |  |  | 0001_init |
| `model_id` | TEXT | no |  |  | 0001_init |
| `billing` | TEXT | no | `'unknown'` |  | 0001_init |
| `tier` | INTEGER | yes |  |  | 0001_init |
| `tier_source` | TEXT | yes |  |  | 0001_init |
| `capabilities_json` | TEXT | no | `'{}'` |  | 0001_init |
| `price_json` | TEXT | yes |  |  | 0001_init |
| `observed_at` | INTEGER | yes |  |  | 0001_init |
| `enabled` | INTEGER | no | `1` |  | 0001_init |
| `context_tokens` | INTEGER | yes |  |  | 0007_auto_routing |
| `max_output_tokens` | INTEGER | yes |  |  | 0007_auto_routing |
| `supports_tools` | INTEGER | yes |  |  | 0007_auto_routing |
| `supports_vision` | INTEGER | yes |  |  | 0007_auto_routing |
| `caps_source` | TEXT | yes |  |  | 0007_auto_routing |
| `caps_observed_at` | INTEGER | yes |  |  | 0007_auto_routing |
| `thinking_types` | TEXT | yes |  |  | 0012_request_features |
| `effort_levels` | TEXT | yes |  |  | 0012_request_features |
| `enabled_source` | TEXT | yes |  |  | 0020_model_enabled_source |

References: `provider_id` → `providers.id` (on delete cascade).

7 CHECK constraints (see the migrations).

## `providers`

| Column | Type | Null | Default | Key | Added in |
| --- | --- | --- | --- | --- | --- |
| `id` | TEXT | yes |  | primary | 0001_init |
| `kind` | TEXT | no |  |  | 0001_init |
| `account_label` | TEXT | no |  |  | 0001_init |
| `endpoint` | TEXT | yes |  |  | 0001_init |
| `enabled` | INTEGER | no | `1` |  | 0001_init |
| `credential_ref` | TEXT | yes |  |  | 0001_init |
| `data_use_terms` | TEXT | no | `'unknown'` |  | 0001_init |
| `terms_source` | TEXT | yes |  |  | 0001_init |
| `terms_checked_at` | INTEGER | yes |  |  | 0001_init |
| `created_at` | INTEGER | no |  |  | 0001_init |
| `default_billing` | TEXT | no | `'unknown'` |  | 0002_gateway |
| `wire` | TEXT | no | `'openai'` |  | 0003_cost_safe_gateway |
| `quota_group` | TEXT | yes |  |  | 0003_cost_safe_gateway |
| `quota_confirmed_at` | INTEGER | yes |  |  | 0004_counter_review |

4 CHECK constraints (see the migrations).

## `rate_limits`

| Column | Type | Null | Default | Key | Added in |
| --- | --- | --- | --- | --- | --- |
| `provider_id` | TEXT | no |  | primary | 0018_rate_limits |
| `model_id` | TEXT | no |  | primary | 0018_rate_limits |
| `headers_json` | TEXT | no |  |  | 0018_rate_limits |
| `http_status` | INTEGER | yes |  |  | 0018_rate_limits |
| `observed_at` | INTEGER | no |  |  | 0018_rate_limits |

## `route_decisions`

| Column | Type | Null | Default | Key | Added in |
| --- | --- | --- | --- | --- | --- |
| `id` | TEXT | yes |  | primary | 0001_init |
| `stage_id` | TEXT | no |  |  | 0001_init |
| `eligible_json` | TEXT | no |  |  | 0001_init |
| `excluded_json` | TEXT | no |  |  | 0001_init |
| `deciding_rule` | TEXT | yes |  |  | 0001_init |
| `chosen` | TEXT | yes |  |  | 0001_init |
| `created_at` | INTEGER | no |  |  | 0001_init |

References: `stage_id` → `task_stages.id` (on delete cascade).

## `session_affinity`

| Column | Type | Null | Default | Key | Added in |
| --- | --- | --- | --- | --- | --- |
| `client_id` | TEXT | no |  | primary | 0003_cost_safe_gateway |
| `session_key` | TEXT | no |  | primary | 0003_cost_safe_gateway |
| `requested_model` | TEXT | no |  | primary | 0003_cost_safe_gateway |
| `provider_id` | TEXT | no |  |  | 0003_cost_safe_gateway |
| `model_id` | TEXT | no |  |  | 0003_cost_safe_gateway |
| `created_at` | INTEGER | no |  |  | 0003_cost_safe_gateway |
| `last_used_at` | INTEGER | no |  |  | 0003_cost_safe_gateway |

References: `provider_id` → `providers.id` (on delete cascade); `client_id` → `gateway_clients.id` (on delete cascade).

Indexes: `sessions_by_last_use`.

## `settings`

| Column | Type | Null | Default | Key | Added in |
| --- | --- | --- | --- | --- | --- |
| `key` | TEXT | yes |  | primary | 0001_init |
| `value` | TEXT | no |  |  | 0001_init |

## `task_stages`

| Column | Type | Null | Default | Key | Added in |
| --- | --- | --- | --- | --- | --- |
| `id` | TEXT | yes |  | primary | 0001_init |
| `task_id` | TEXT | no |  |  | 0001_init |
| `position` | INTEGER | no |  |  | 0001_init |
| `name` | TEXT | no |  |  | 0001_init |
| `check_spec` | TEXT | yes |  |  | 0001_init |
| `state` | TEXT | no |  |  | 0001_init |

References: `task_id` → `tasks.id` (on delete cascade).

## `tasks`

| Column | Type | Null | Default | Key | Added in |
| --- | --- | --- | --- | --- | --- |
| `id` | TEXT | yes |  | primary | 0001_init |
| `workspace_id` | TEXT | no |  |  | 0001_init |
| `title` | TEXT | no |  |  | 0001_init |
| `prompt` | TEXT | no |  |  | 0001_init |
| `strategy` | TEXT | no |  |  | 0001_init |
| `state` | TEXT | no |  |  | 0001_init |
| `base_revision` | TEXT | yes |  |  | 0001_init |
| `policy_snapshot_json` | TEXT | no | `'{}'` |  | 0001_init |
| `created_at` | INTEGER | no |  |  | 0001_init |
| `updated_at` | INTEGER | no |  |  | 0001_init |
| `phase` | TEXT | yes |  |  | 0008_task_engine |
| `reason` | TEXT | yes |  |  | 0008_task_engine |
| `result` | TEXT | yes |  |  | 0008_task_engine |
| `result_branch` | TEXT | yes |  |  | 0008_task_engine |
| `client_id` | TEXT | yes |  |  | 0009_tasks_from_clients |

References: `workspace_id` → `workspaces.id`.

Indexes: `tasks_by_client`, `tasks_by_workspace`.

1 CHECK constraint (see the migrations).

## `tool_signatures`

| Column | Type | Null | Default | Key | Added in |
| --- | --- | --- | --- | --- | --- |
| `call_id` | TEXT | yes |  | primary | 0016_tool_signatures |
| `signature` | TEXT | no |  |  | 0016_tool_signatures |
| `created_at` | INTEGER | no |  |  | 0016_tool_signatures |

Indexes: `tool_signatures_by_age`.

## `usage_events`

| Column | Type | Null | Default | Key | Added in |
| --- | --- | --- | --- | --- | --- |
| `id` | TEXT | yes |  | primary | 0001_init |
| `attempt_id` | TEXT | yes |  |  | 0001_init |
| `client_id` | TEXT | yes |  |  | 0001_init |
| `provider` | TEXT | no |  |  | 0001_init |
| `model` | TEXT | no |  |  | 0001_init |
| `input_tokens` | INTEGER | yes |  |  | 0001_init |
| `output_tokens` | INTEGER | yes |  |  | 0001_init |
| `cache_tokens` | INTEGER | yes |  |  | 0001_init |
| `cost_micro_usd` | INTEGER | yes |  |  | 0001_init |
| `cost_status` | TEXT | no |  |  | 0001_init |
| `source` | TEXT | no |  |  | 0001_init |
| `created_at` | INTEGER | no |  |  | 0001_init |
| `outcome` | TEXT | no | `'ok'` |  | 0002_gateway |
| `http_status` | INTEGER | yes |  |  | 0002_gateway |
| `latency_ms` | INTEGER | yes |  |  | 0002_gateway |
| `requested_model` | TEXT | yes |  |  | 0002_gateway |
| `error` | TEXT | yes |  |  | 0002_gateway |
| `cache_write_tokens` | INTEGER | yes |  |  | 0003_cost_safe_gateway |
| `session_key` | TEXT | yes |  |  | 0003_cost_safe_gateway |
| `route_mode` | TEXT | yes |  |  | 0003_cost_safe_gateway |
| `at_risk_micro_usd` | INTEGER | yes |  |  | 0004_counter_review |
| `route_reason` | TEXT | yes |  |  | 0007_auto_routing |

References: `attempt_id` → `attempts.id`.

Indexes: `usage_by_attempt`, `usage_by_client`, `usage_by_time`.

3 CHECK constraints (see the migrations).

## `workspace_checks`

| Column | Type | Null | Default | Key | Added in |
| --- | --- | --- | --- | --- | --- |
| `id` | TEXT | yes |  | primary | 0001_init |
| `workspace_id` | TEXT | no |  |  | 0001_init |
| `command` | TEXT | no |  |  | 0001_init |
| `source` | TEXT | no |  |  | 0001_init |
| `approved` | INTEGER | no | `0` |  | 0001_init |

References: `workspace_id` → `workspaces.id` (on delete cascade).

1 CHECK constraint (see the migrations).

## `workspaces`

| Column | Type | Null | Default | Key | Added in |
| --- | --- | --- | --- | --- | --- |
| `id` | TEXT | yes |  | primary | 0001_init |
| `name` | TEXT | no |  |  | 0001_init |
| `path` | TEXT | no |  |  | 0001_init |
| `privacy_class` | TEXT | no | `'standard'` |  | 0001_init |
| `isolation` | TEXT | no | `'git_worktree'` |  | 0001_init |
| `created_at` | INTEGER | no |  |  | 0001_init |
| `daily_budget_micro_usd` | INTEGER | yes |  |  | 0011_task_budgets |

2 CHECK constraints (see the migrations).
