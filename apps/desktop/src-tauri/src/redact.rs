//! Removes secrets from text before it is stored or shown: process output, check output, traces
//! (plan §7). Known secrets are replaced exactly; common API-key shapes are replaced by pattern.

pub const MASK: &str = "[redacted]";

/// Key prefixes used by common providers. A token starting with one of these and long enough to be a key
/// is masked even when it is not one of the known secrets.
const KEY_PREFIXES: &[&str] = &["sk-", "sk_", "rk_", "pk_live_", "ghp_", "gho_", "github_pat_", "xoxb-", "xoxp-", "AIza"];
const MIN_KEY_LEN: usize = 20;

pub fn redact(text: &str, known_secrets: &[&str]) -> String {
    let mut out = text.to_string();
    for secret in known_secrets.iter().filter(|s| s.len() >= 8) {
        out = out.replace(secret, MASK);
    }
    mask_key_shapes(&out)
}

fn is_key_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

fn mask_key_shapes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        // A key starts at a token boundary.
        let start = rest
            .char_indices()
            .find(|&(i, _)| {
                let boundary = i == 0 || !is_key_char(rest[..i].chars().next_back().unwrap_or(' '));
                boundary && KEY_PREFIXES.iter().any(|p| rest[i..].starts_with(p))
            })
            .map(|(i, _)| i);
        let Some(start) = start else {
            out.push_str(rest);
            break;
        };
        let token_len = rest[start..].chars().take_while(|&c| is_key_char(c)).map(char::len_utf8).sum::<usize>();
        out.push_str(&rest[..start]);
        if token_len >= MIN_KEY_LEN {
            out.push_str(MASK);
        } else {
            out.push_str(&rest[start..start + token_len]);
        }
        rest = &rest[start + token_len.max(1)..];
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_known_secrets() {
        assert_eq!(redact("token=hunter2hunter2 ok", &["hunter2hunter2"]), "token=[redacted] ok");
    }

    #[test]
    fn masks_key_shapes() {
        let text = "Authorization: Bearer sk-or-v1-0123456789abcdef0123 end";
        assert_eq!(redact(text, &[]), "Authorization: Bearer [redacted] end");
    }

    #[test]
    fn leaves_short_and_embedded_tokens() {
        // Too short to be a key, and "task-" is not a prefix match at a boundary.
        assert_eq!(redact("sk-short and task-sk-0123456789abcdefghij", &[]), "sk-short and task-sk-0123456789abcdefghij");
    }

    #[test]
    fn ignores_trivially_short_known_secrets() {
        assert_eq!(redact("a b c", &["a"]), "a b c");
    }

    #[test]
    fn handles_unicode_around_keys() {
        let text = "khóa: sk-0123456789abcdefghijkl — xong";
        assert_eq!(redact(text, &[]), "khóa: [redacted] — xong");
    }
}
