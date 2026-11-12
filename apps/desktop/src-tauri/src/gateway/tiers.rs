//! Pygg's tier table (ROADMAP B3, `curated:v2`): a starting tier for models the user has not classed, from the
//! vendor's own naming of its model line. A tier is a judgement, not a measurement: the table says which part of a
//! vendor's line a model belongs to (the smallest, the middle, the top), and nothing more. Models it does not know
//! keep no tier, and auto skips them. A tier the user sets or clears is theirs; the table never changes it.
//!
//! T0 runs on this computer, T1 is a vendor's small and fast line, T2 its capable low-cost line, T3 its strong
//! coding and reasoning line, T4 its frontier line.

/// The table's version, written as the tier's source. v2 (2026-11-12): Claude Haiku 5 moved from T1 to T2 on the P0
/// benchmark's evidence. Tiers an earlier version filled are brought up to date; the user's are never touched.
pub const SOURCE: &str = "curated:v2";

/// Connection kinds that run models on this computer.
const LOCAL: &[&str] = &["ollama", "lmstudio", "llamacpp", "vllm", "docker-model-runner", "jan"];

/// (pattern, tier, why) in order: the first whose every part appears in the model id wins, so the narrower line
/// comes first (`flash-lite` before `flash`, `mini` before the base model).
const LINES: &[(&[&str], &str, &str)] = &[
    // Anthropic: Haiku, Sonnet, Opus; Fable and Mythos sit above Opus.
    (&["claude", "fable"], "T4", "Anthropic's top line (Fable)"),
    (&["claude", "mythos"], "T4", "Anthropic's top line (Mythos)"),
    (&["claude", "opus"], "T4", "Anthropic's Opus line"),
    (&["claude", "sonnet"], "T3", "Anthropic's Sonnet line, the middle of Haiku, Sonnet and Opus"),
    // Measured, not only named: with OpenCode and with Claude Code, Haiku 5.5 solved as many of the P0 benchmark's
    // 12 harder coding tasks as Sonnet 5.5 (benchmark/reports/2026-11-12-p0-optimization.md).
    (&["claude", "haiku-5"], "T2", "Anthropic's Haiku 5 line; on Pygg's P0 coding benchmark it solved as many tasks as Sonnet 5.5"),
    (&["claude", "haiku"], "T1", "Anthropic's Haiku line, the smallest of its three"),
    // Google Gemini: Flash-Lite, Flash, Pro.
    (&["gemini", "flash-lite"], "T1", "Google's Flash-Lite line, the smallest Gemini"),
    (&["gemini", "flash"], "T2", "Google's Flash line, between Flash-Lite and Pro"),
    (&["gemini", "pro"], "T3", "Google's Pro line, the largest Gemini"),
    // OpenAI GPT: nano, mini, then the base model.
    (&["gpt-", "nano"], "T1", "OpenAI's nano size, the smallest of a GPT generation"),
    (&["gpt-", "mini"], "T2", "OpenAI's mini size"),
    (&["gpt-oss-20b"], "T1", "OpenAI's smaller open-weight model"),
    (&["gpt-oss-120b"], "T2", "OpenAI's larger open-weight model"),
    (&["gpt-5"], "T3", "OpenAI's full-size GPT-5"),
    (&["gpt-4.1"], "T3", "OpenAI's full-size GPT-4.1"),
    // DeepSeek: chat and reasoner.
    (&["deepseek-reasoner"], "T3", "DeepSeek's reasoning model"),
    (&["deepseek-chat"], "T2", "DeepSeek's chat model"),
    // Mistral: small, medium, large.
    (&["mistral-large"], "T3", "Mistral's large line"),
    (&["mistral-medium"], "T2", "Mistral's medium line"),
    (&["mistral-small"], "T1", "Mistral's small line"),
    (&["codestral"], "T2", "Mistral's coding model"),
    // Qwen (Alibaba): turbo/flash, plus, max.
    (&["qwen", "max"], "T3", "Qwen's Max line, the largest"),
    (&["qwen", "plus"], "T2", "Qwen's Plus line"),
    (&["qwen", "turbo"], "T1", "Qwen's Turbo line, the smallest"),
    (&["qwen", "flash"], "T1", "Qwen's Flash line, the smallest"),
    // Meta Llama by size.
    (&["llama", "70b"], "T2", "a 70-billion-parameter Llama"),
    (&["llama", "8b"], "T1", "an 8-billion-parameter Llama"),
    (&["llama", "3b"], "T1", "a 3-billion-parameter Llama"),
];

/// A starting tier for a model and why, or `None` when the table does not know its line.
pub fn suggest(provider_kind: &str, model_id: &str) -> Option<(&'static str, &'static str)> {
    if LOCAL.contains(&provider_kind) {
        return Some(("T0", "runs on this computer"));
    }
    let id = model_id.to_ascii_lowercase();
    LINES.iter().find(|(parts, _, _)| parts.iter().all(|p| id.contains(p))).map(|(_, tier, why)| (*tier, *why))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_models_by_the_vendors_own_line() {
        let tier = |kind: &str, id: &str| suggest(kind, id).map(|(t, _)| t);
        assert_eq!(tier("anthropic", "claude-haiku-4-5"), Some("T1"));
        assert_eq!(tier("anthropic", "claude-haiku-5-5"), Some("T2"), "measured on the P0 benchmark");
        assert_eq!(tier("openrouter", "anthropic/claude-haiku-5.5"), Some("T2"));
        assert_eq!(tier("anthropic", "claude-sonnet-5-5"), Some("T3"));
        assert_eq!(tier("anthropic", "claude-opus-5-5"), Some("T4"));
        assert_eq!(tier("openrouter", "anthropic/claude-sonnet-4.5"), Some("T3"), "a router's vendor prefix is fine");
        assert_eq!(tier("gemini", "gemini-3.1-flash-lite"), Some("T1"), "Flash-Lite before Flash");
        assert_eq!(tier("gemini", "gemini-2.5-flash"), Some("T2"));
        assert_eq!(tier("gemini", "gemini-3.1-pro-preview"), Some("T3"));
        assert_eq!(tier("openai", "gpt-4.1-mini"), Some("T2"), "mini before the base model");
        assert_eq!(tier("openai", "gpt-5-nano"), Some("T1"));
        assert_eq!(tier("openai", "gpt-5"), Some("T3"));
        assert_eq!(tier("groq", "openai/gpt-oss-120b"), Some("T2"));
        assert_eq!(tier("deepseek", "deepseek-reasoner"), Some("T3"));
        assert_eq!(tier("groq", "llama-3.3-70b-versatile"), Some("T2"));
        assert_eq!(tier("ollama", "qwen2.5-coder:7b"), Some("T0"), "anything local is T0");
        assert_eq!(tier("openrouter", "moonshotai/kimi-k2"), None, "a line the table does not know stays unset");
    }
}
