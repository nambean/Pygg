//! Connection presets: providers with an OpenAI-compatible chat completions API. A preset only fills in
//! defaults; the user can change the endpoint and billing type of any connection.

use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    /// `None` for Custom, where the user must give one.
    pub endpoint: Option<&'static str>,
    pub needs_key: bool,
    /// What a new connection is assumed to be until the user says otherwise. `unknown` where it depends on
    /// the account (a Gemini key can be free tier or paid).
    pub default_billing: &'static str,
    /// Some providers list models as `models/<id>` but take `<id>` in requests.
    #[serde(skip)]
    pub strip_model_prefix: Option<&'static str>,
}

pub const PRESETS: &[Preset] = &[
    Preset { id: "gemini", name: "Google Gemini", endpoint: Some("https://generativelanguage.googleapis.com/v1beta/openai"),
        needs_key: true, default_billing: "unknown", strip_model_prefix: Some("models/") },
    Preset { id: "openrouter", name: "OpenRouter", endpoint: Some("https://openrouter.ai/api/v1"),
        needs_key: true, default_billing: "paid", strip_model_prefix: None },
    Preset { id: "openai", name: "OpenAI", endpoint: Some("https://api.openai.com/v1"),
        needs_key: true, default_billing: "paid", strip_model_prefix: None },
    Preset { id: "groq", name: "Groq", endpoint: Some("https://api.groq.com/openai/v1"),
        needs_key: true, default_billing: "unknown", strip_model_prefix: None },
    Preset { id: "deepseek", name: "DeepSeek", endpoint: Some("https://api.deepseek.com/v1"),
        needs_key: true, default_billing: "paid", strip_model_prefix: None },
    Preset { id: "ollama", name: "Ollama", endpoint: Some("http://127.0.0.1:11434/v1"),
        needs_key: false, default_billing: "local", strip_model_prefix: None },
    Preset { id: "custom", name: "Custom (OpenAI-compatible)", endpoint: None,
        needs_key: false, default_billing: "unknown", strip_model_prefix: None },
];

pub fn find(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}
