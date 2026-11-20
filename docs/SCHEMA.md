# Store schema

> **Status:** Current · generated from the migrations by `scripts/schema-reference.py`; do not edit by hand · schema v45

The local SQLite store (`db.rs`): every table after all migrations, in order. Credentials are never here: a
connection holds only a reference to its key in the OS credential store (ADR-002). Client tokens are stored as
SHA-256 hashes.

## `approvals`

Added in 0001_init. Columns: `id` (PK), `attempt_id` (FK `attempts`), `cap_micro_usd`, `estimate_low_micro`, `estimate_high_micro`, `policy_version`, `state`, `created_at`, `decided_at`.

## `attempts`

Added in 0001_init. Columns: `id` (PK), `stage_id` (FK `task_stages`), `number`, `agent`, `model_ref`, `tier`, `billing`, `worktree_path`, `state`, `check_results_json`, `escalation_reason`, `started_at`, `finished_at`, `rung`, `base_revision`, `agent_model`, `detail`, `files_changed`, `insertions`, `deletions`, `summary`, `log_tail`, `agent_estimate_micro_usd`, `handoff_sent`, `changed_paths_json`, `cost_source`.

## `audit_events`

Added in 0001_init. Columns: `id` (PK), `actor`, `action`, `entity`, `metadata_json`, `created_at`.

## `board_cards`

Added in 0044_board. Columns: `id` (PK), `workspace_id` (FK `workspaces`), `brainstorm_id` (FK `brainstorms`), `card_id`, `title`, `role`, `done`, `prompt`, `depends_on_json`, `ladder_json`, `agent`, `runner`, `class`, `lane`, `held`, `task_id` (FK `tasks`), `blocked_reason`, `blocked_since`, `blocked_ms`, `ready_at`, `started_at`, `done_at`, `position`, `created_at`, `updated_at`.

## `board_settings`

Added in 0044_board. Columns: `workspace_id` (PK, FK `workspaces`), `wip_running`, `wip_checking`, `updated_at`.

## `brain_picks`

Added in 0042_brainstorms. Columns: `scope` (PK), `role` (PK), `provider_id`, `model_id`, `saved_at`.

## `brainstorms`

Added in 0042_brainstorms. Columns: `id` (PK), `client_id`, `session`, `workspace_id` (FK `workspaces`), `state`, `requirement`, `planner`, `prompt_version`, `revision`, `proposal_json`, `raw_reply`, `note`, `error`, `created_at`, `updated_at`, `branch`.

## `budget_reservations`

Added in 0003_cost_safe_gateway. Columns: `id` (PK), `client_id` (FK `gateway_clients`), `reserved_micro`, `created_at`, `provider_id`, `model_id`, `requested_model`, `session_key`, `route_mode`, `owner_generation`.

## `combo_members`

Added in 0002_gateway. Columns: `combo_id` (PK, FK `combos`), `position` (PK), `provider_id` (FK `providers`), `model_id`.

## `combos`

Added in 0002_gateway. Columns: `id` (PK), `name`, `created_at`, `strategy`.

## `connection_checks`

Added in 0029_connection_checks. Columns: `provider_id` (FK `providers`), `at`, `ok`, `kind`, `detail`.

## `connection_health`

Added in 0003_cost_safe_gateway. Columns: `provider_id` (PK, FK `providers`), `model_id` (PK), `state`, `reason`, `http_status`, `until_ms`, `updated_at`, `strikes`.

## `feedback`

Added in 0015_quality_signals. Columns: `id` (PK), `metric_name` (FK `metrics`), `target_kind`, `target_id`, `value_bool`, `value_float`, `source`, `tags_json`, `created_at`.

## `gateway_clients`

Added in 0002_gateway. Columns: `id` (PK), `name`, `token_hash`, `token_prefix`, `monthly_budget_micro_usd`, `created_at`, `last_used_at`, `revoked_at`, `max_concurrency`, `unknown_cost_policy`, `allowed_connections`, `paid_retries`, `auto_min_tier`, `may_run_tasks`, `require_no_training`, `strict_budget`, `token_ref`, `deleted_at`, `session_budget_micro_usd`, `day_budget_micro_usd`.

## `key_health`

Added in 0031_key_rings. Columns: `key_id` (PK, FK `provider_keys`), `model_id` (PK), `state`, `reason`, `http_status`, `until_ms`, `updated_at`.

## `key_limits`

Added in 0031_key_rings. Columns: `key_id` (PK, FK `provider_keys`), `model_id` (PK), `headers_json`, `http_status`, `observed_at`.

## `launches`

Added in 0038_launches. Columns: `session_key` (PK), `tool`, `model`, `started_at`, `finished_at`, `exit_code`, `repo`, `branch`, `head_before`, `head_after`, `dirty_before`, `dirty_after`, `commits`, `files_changed`, `insertions`, `deletions`.

## `metrics`

Added in 0015_quality_signals. Columns: `name` (PK), `type`, `level`, `optimize`, `description`.

## `models`

Added in 0001_init. Columns: `id` (PK), `provider_id` (FK `providers`), `model_id`, `billing`, `tier`, `tier_source`, `capabilities_json`, `price_json`, `observed_at`, `enabled`, `context_tokens`, `max_output_tokens`, `supports_tools`, `supports_vision`, `caps_source`, `caps_observed_at`, `thinking_types`, `effort_levels`, `enabled_source`.

## `provider_keys`

Added in 0031_key_rings. Columns: `id` (PK), `provider_id` (FK `providers`), `label`, `credential_ref`, `rank`, `enabled`, `separate_account_at`, `created_at`.

## `providers`

Added in 0001_init. Columns: `id` (PK), `kind`, `account_label`, `endpoint`, `enabled`, `credential_ref`, `data_use_terms`, `terms_source`, `terms_checked_at`, `created_at`, `default_billing`, `wire`, `quota_group`, `quota_confirmed_at`, `pool`, `pool_rank`, `web_profile`, `web_ack_at`, `web_ack_version`, `web_state`, `web_state_reason`, `web_state_at`, `web_daily_limit`, `web_next_at`, `web_backoff_until`, `web_backoff_reason`, `web_strikes`, `key_strategy`, `key_cursor`, `web_account`.

## `rate_limits`

Added in 0018_rate_limits. Columns: `provider_id` (PK), `model_id` (PK), `headers_json`, `http_status`, `observed_at`.

## `route_decisions`

Added in 0001_init. Columns: `id` (PK), `stage_id` (FK `task_stages`), `eligible_json`, `excluded_json`, `deciding_rule`, `chosen`, `created_at`.

## `session_affinity`

Added in 0003_cost_safe_gateway. Columns: `client_id` (PK, FK `gateway_clients`), `session_key` (PK), `requested_model` (PK), `provider_id` (FK `providers`), `model_id`, `created_at`, `last_used_at`, `home_provider_id`, `home_model_id`, `home_used_at`.

## `settings`

Added in 0001_init. Columns: `key` (PK), `value`.

## `skill_projects`

Added in 0035_skill_projects. Columns: `path` (PK), `added_at`.

## `skills`

Added in 0032_skills. Columns: `id` (PK), `description`, `body`, `manual`, `created_at`, `updated_at`.

## `task_stages`

Added in 0001_init. Columns: `id` (PK), `task_id` (FK `tasks`), `position`, `name`, `check_spec`, `state`.

## `tasks`

Added in 0001_init. Columns: `id` (PK), `workspace_id` (FK `workspaces`), `title`, `prompt`, `strategy`, `state`, `base_revision`, `policy_snapshot_json`, `created_at`, `updated_at`, `phase`, `reason`, `result`, `result_branch`, `client_id`, `brainstorm_id`, `card_id`, `card_role`.

## `tool_signatures`

Added in 0016_tool_signatures. Columns: `call_id` (PK), `signature`, `created_at`.

## `usage_events`

Added in 0001_init. Columns: `id` (PK), `attempt_id` (FK `attempts`), `client_id`, `provider`, `model`, `input_tokens`, `output_tokens`, `cache_tokens`, `cost_micro_usd`, `cost_status`, `source`, `created_at`, `outcome`, `http_status`, `latency_ms`, `requested_model`, `error`, `cache_write_tokens`, `session_key`, `route_mode`, `at_risk_micro_usd`, `route_reason`, `repeated_tokens`, `request_id`, `attempt`, `billing`, `route_kind`, `cache_hint`, `key_id`, `cache_miss`, `tools_tokens`, `system_tokens`, `prefix_hash`, `clear_count`, `clear_tokens`, `clear_tail_tokens`, `request_class`, `agent_type`, `agent_id`, `prompt_id`.

## `web_pace`

Added in 0028_web_guard. Columns: `provider_id` (FK `providers`), `at`, `weight`, `outcome`.

## `workspace_checks`

Added in 0001_init. Columns: `id` (PK), `workspace_id` (FK `workspaces`), `command`, `source`, `approved`.

## `workspaces`

Added in 0001_init. Columns: `id` (PK), `name`, `path`, `privacy_class`, `isolation`, `created_at`, `daily_budget_micro_usd`.
