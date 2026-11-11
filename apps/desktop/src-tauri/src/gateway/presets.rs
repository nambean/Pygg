//! Connection presets. A preset only fills in defaults; the user can change the endpoint and billing type of
//! any connection. `wire` is the API the provider speaks: requests in the same format are passed through
//! unchanged (ROADMAP P0-1), and other pairs are translated only where a contract test exists.

use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    /// `None` for the Custom presets, where the user must give one. Includes the version path (`…/v1`).
    pub endpoint: Option<&'static str>,
    pub needs_key: bool,
    /// What a new connection is assumed to be until the user says otherwise. `unknown` where it depends on
    /// the account (a Gemini key can be free tier or paid).
    pub default_billing: &'static str,
    /// `openai` (Chat Completions) or `anthropic` (Messages).
    pub wire: &'static str,
    /// Some providers list models as `models/<id>` but take `<id>` in requests.
    #[serde(skip)]
    pub strip_model_prefix: Option<&'static str>,
    /// The provider accepts Anthropic-style `cache_control` markers on OpenAI-format content parts, so a
    /// translated Anthropic request can keep its prompt-cache hints (OpenRouter documents this).
    #[serde(skip)]
    pub accepts_cache_control: bool,
}

const fn preset(id: &'static str, name: &'static str, endpoint: Option<&'static str>, needs_key: bool,
    default_billing: &'static str, wire: &'static str) -> Preset {
    Preset { id, name, endpoint, needs_key, default_billing, wire, strip_model_prefix: None, accepts_cache_control: false }
}

pub const PRESETS: &[Preset] = &[
    Preset { strip_model_prefix: Some("models/"),
        ..preset("gemini", "Google Gemini", Some("https://generativelanguage.googleapis.com/v1beta/openai"), true, "unknown", "openai") },
    preset("anthropic", "Anthropic", Some("https://api.anthropic.com/v1"), true, "paid", "anthropic"),
    Preset { accepts_cache_control: true,
        ..preset("openrouter", "OpenRouter", Some("https://openrouter.ai/api/v1"), true, "paid", "openai") },
    preset("openai", "OpenAI", Some("https://api.openai.com/v1"), true, "paid", "openai"),
    preset("groq", "Groq", Some("https://api.groq.com/openai/v1"), true, "unknown", "openai"),
    preset("deepseek", "DeepSeek", Some("https://api.deepseek.com/v1"), true, "paid", "openai"),
    // Alibaba Cloud Model Studio's OpenAI-compatible mode (international). Plans with their own endpoint, such
    // as a token plan, set it when connecting.
    preset("qwen", "Qwen (Alibaba Model Studio)", Some("https://dashscope-intl.aliyuncs.com/compatible-mode/v1"), true, "unknown", "openai"),
    preset("ollama", "Ollama", Some("http://127.0.0.1:11434/v1"), false, "local", "openai"),
    preset("custom", "Custom (OpenAI-compatible)", None, false, "unknown", "openai"),
    preset("custom-anthropic", "Custom (Anthropic-compatible)", None, false, "unknown", "anthropic"),
];

pub fn find(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}
