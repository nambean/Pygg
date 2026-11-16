//! Codex's model catalog for the models Pygg lists (ROADMAP item 5).
//!
//! For a model it does not know, Codex uses fallback metadata: its default instructions, a 272,000-token context and
//! no `apply_patch` tool, so it edits files through the shell and compacts at the wrong size. A profile may name a
//! catalog file (`model_catalog_json`); Codex then knows only its models. Pygg writes one next to Codex's `config.toml`,
//! `pygg-models.json`, and its `pygg` profile names it, so only `codex --profile pygg` (and `pygg run codex`) read it.
//! It is built from the same list OpenCode gets: each model keeps the fallback's values, Codex's own default
//! instructions included, and gains its real context window, the freeform `apply_patch` tool and its input kinds.
//! Nothing else about Codex's requests changes.
//!
//! A file, not an address the gateway serves: Codex refuses a fetched catalog over 1 MiB (`MAX_MODEL_CATALOG_BYTES`),
//! and with 21 KB of instructions in each model a list of 50 would pass it. A file it reads has no such limit.

use crate::integrations::GatewayModel;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// Codex's default instructions for a model it does not know (`codex-rs/models-manager/prompt.md` of Codex CLI 0.163.0,
/// Apache-2.0: `third_party/codex/`, credited in THIRD_PARTY_NOTICES.md). Every model in the catalog carries them, so a
/// model keeps the instructions Codex gives it without a catalog. Owner's decision, 2026-11-16.
pub const CODEX_INSTRUCTIONS: &str = include_str!("../third_party/codex/prompt.md");

/// The Codex version those instructions come from.
pub const CODEX_VERSION: &str = "0.163.0";

/// The context window Codex assumes for a model it does not know; kept for `auto`, whose model is chosen per session.
pub const FALLBACK_CONTEXT: u64 = 272_000;

/// The catalog's file, next to Codex's `config.toml`.
pub fn path(config: &Path) -> PathBuf {
    config.with_file_name("pygg-models.json")
}

/// One model, as Codex's fallback metadata describes an unknown one (`model_info_from_slug`), with what Pygg knows.
fn entry(model: &GatewayModel, priority: usize) -> Value {
    let context = model.context_tokens.unwrap_or(FALLBACK_CONTEXT);
    let inputs = if model.supports_vision == Some(false) { json!(["text"]) } else { json!(["text", "image"]) };
    json!({
        "slug": model.id,
        "display_name": model.id,
        "description": null,
        "supported_reasoning_levels": [],
        "shell_type": "unified_exec",
        "visibility": "list",
        "supported_in_api": true,
        "priority": priority,
        "availability_nux": null,
        "upgrade": null,
        "base_instructions": CODEX_INSTRUCTIONS,
        "include_apps_usage_instructions": false,
        "default_reasoning_summary": "auto",
        "support_verbosity": false,
        "default_verbosity": null,
        // A model that takes tools gets Codex's own patch tool; it crosses the gateway as a function with one string.
        "apply_patch_tool_type": if model.supports_tools == Some(false) { Value::Null } else { json!("freeform") },
        "truncation_policy": { "mode": "bytes", "limit": 10_000 },
        "context_window": context,
        "max_context_window": context,
        "effective_context_window_percent": 95,
        "experimental_supported_tools": [],
        "input_modalities": inputs,
    })
}

/// The catalog for these models, as Codex reads a `/models` answer.
pub fn catalog(models: &[GatewayModel]) -> Value {
    json!({ "models": models.iter().enumerate().map(|(i, m)| entry(m, i)).collect::<Vec<_>>() })
}

/// The catalog file's text. Codex refuses to start on a catalog it cannot read, so it is always written whole.
pub fn text(models: &[GatewayModel]) -> String {
    format!("{}\n", catalog(models))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model(id: &str, context: Option<u64>, tools: Option<bool>, vision: Option<bool>) -> GatewayModel {
        GatewayModel {
            id: id.into(),
            context_tokens: context,
            max_output_tokens: None,
            supports_tools: tools,
            supports_vision: vision,
        }
    }

    #[test]
    fn each_model_keeps_codexs_own_instructions_and_gains_its_context_and_apply_patch() {
        let list = [
            model("auto", None, Some(true), None),
            model("openrouter/anthropic/claude-haiku-5.5", Some(1_000_000), Some(true), Some(true)),
            model("ollama/qwen3:8b", Some(40_960), None, Some(false)),
        ];
        let catalog = catalog(&list);
        let models = catalog["models"].as_array().unwrap();
        assert_eq!(models.len(), 3);
        for (i, m) in models.iter().enumerate() {
            assert_eq!(m["base_instructions"].as_str(), Some(CODEX_INSTRUCTIONS));
            assert_eq!(m["priority"], i);
            assert_eq!((m["visibility"].as_str(), m["shell_type"].as_str()), (Some("list"), Some("unified_exec")));
        }
        assert!(CODEX_INSTRUCTIONS.starts_with("You are a coding agent running in the Codex CLI"));
        assert!(!CODEX_INSTRUCTIONS.contains('\r'), "kept byte for byte, without a checkout's CRLF");
        assert_eq!(models[0]["context_window"], FALLBACK_CONTEXT);
        assert_eq!(models[1]["context_window"], 1_000_000);
        assert_eq!(models[1]["apply_patch_tool_type"], "freeform");
        assert_eq!(models[1]["input_modalities"], json!(["text", "image"]));
        // Unknown tool support is not taken as none; a model known to see no images says so.
        assert_eq!(models[2]["apply_patch_tool_type"], "freeform");
        assert_eq!(models[2]["input_modalities"], json!(["text"]));
        assert_eq!(models[2]["max_context_window"], 40_960);
    }
}
