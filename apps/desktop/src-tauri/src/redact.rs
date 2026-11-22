//! Removes secrets from text before it is stored or shown: process output, check output, traces
//! (plan §7). Known secrets are replaced exactly; common API-key shapes are replaced by pattern.

pub const MASK: &str = "[redacted]";

/// Key prefixes used by common providers. A token starting with one of these and long enough to be a key
/// is masked even when it is not one of the known secrets.
const KEY_PREFIXES: &[&str] = &[
    "sk-",
    "sk_",
    "rk_",
    "pk_live_",
    "ghp_",
    "gho_",
    "ghs_",
    "ghu_",
    "github_pat_",
    "glpat-",
    "xoxb-",
    "xoxp-",
    "xoxa-",
    "xoxs-",
    "AIza",
    "AKIA",
    "ASIA",
    "hf_",
    "npm_",
    "eyJ",
];
const MIN_KEY_LEN: usize = 20;

/// Names whose value is a secret when assigned (`NAME=value`, `name: "value"`), matched in upper case.
const SECRET_NAMES: &[&str] =
    &["PASSWORD", "PASSWD", "SECRET", "TOKEN", "API_KEY", "APIKEY", "ACCESS_KEY", "PRIVATE_KEY", "ACCOUNTKEY"];
/// A value shorter than this is left alone: `token: 5`, `password: none`.
const MIN_VALUE_LEN: usize = 8;

pub fn redact(text: &str, known_secrets: &[&str]) -> String {
    let mut out = text.to_string();
    for secret in known_secrets.iter().filter(|s| s.len() >= 8) {
        out = out.replace(secret, MASK);
    }
    let out = mask_private_keys(&out);
    let out = mask_url_credentials(&out);
    let out = mask_after_words(&out);
    mask_key_shapes(&out)
}

/// A PEM private key block, from its BEGIN line to its END line.
fn mask_private_keys(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("-----BEGIN ") {
        let header_end = rest[start..].find("-----\n").or_else(|| rest[start + 11..].find("-----").map(|i| i + 11));
        let is_private = header_end.is_some_and(|e| rest[start..start + e].contains("PRIVATE KEY"));
        if !is_private {
            out.push_str(&rest[..start + 11]);
            rest = &rest[start + 11..];
            continue;
        }
        out.push_str(&rest[..start]);
        out.push_str(MASK);
        match rest[start..].find("-----END ") {
            Some(end) => {
                let after = start + end + 9;
                let close = rest[after..].find("-----").map(|i| after + i + 5).unwrap_or(rest.len());
                rest = &rest[close..];
            }
            None => rest = "",
        }
    }
    out.push_str(rest);
    out
}

/// `scheme://user:password@host` → `scheme://[redacted]@host`.
fn mask_url_credentials(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("://") {
        let (head, tail) = rest.split_at(at + 3);
        out.push_str(head);
        let end = tail
            .find(|c: char| c.is_whitespace() || c == '/' || c == '`' || c == ')' || c == '"' || c == '\'')
            .unwrap_or(tail.len());
        match tail[..end].rfind('@') {
            Some(i) if tail[..i].contains(':') => {
                out.push_str(MASK);
                rest = &tail[i..];
            }
            _ => rest = tail,
        }
    }
    out.push_str(rest);
    out
}

/// The value after `Bearer ` or `Basic `, and after a secret-sounding name assigned with `=` or `:`.
fn mask_after_words(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        out.push_str(&mask_line(line));
    }
    out
}

fn mask_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    loop {
        let Some((value_start, value_end)) = next_secret_value(rest) else {
            out.push_str(rest);
            return out;
        };
        out.push_str(&rest[..value_start]);
        out.push_str(MASK);
        rest = &rest[value_end..];
    }
}

/// The byte range of the next secret value in a line, if any.
fn next_secret_value(line: &str) -> Option<(usize, usize)> {
    let upper = line.to_ascii_uppercase();
    let mut best: Option<(usize, usize)> = None;
    let mut consider = |start: usize| {
        let bytes = line.as_bytes();
        let mut s = start;
        while s < bytes.len() && (bytes[s] == b' ' || bytes[s] == b'"' || bytes[s] == b'\'') {
            s += 1;
        }
        let e = line[s..]
            .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ';' || c == ',' || c == '&' || c == '`')
            .map(|i| s + i)
            .unwrap_or(line.len());
        // A count (`input_tokens: 123456789`) is not a secret: a value needs a letter or a symbol.
        let value = &line[s..e];
        if e - s >= MIN_VALUE_LEN
            && !value.starts_with(MASK)
            && !value.chars().all(|c| c.is_ascii_digit())
            && best.is_none_or(|(b, _)| s < b)
        {
            best = Some((s, e));
        }
    };
    for word in ["BEARER ", "BASIC "] {
        if let Some(i) = upper.find(word) {
            consider(i + word.len());
        }
    }
    for name in SECRET_NAMES {
        let mut from = 0;
        while let Some(i) = upper[from..].find(name).map(|i| from + i) {
            // The name may be followed by more of the identifier (`_ID`, `"`), then `=` or `:`.
            let after = &line[i + name.len()..];
            let skip = after
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '"' || c == '\''))
                .unwrap_or(after.len());
            let tail = &after[skip..];
            let trimmed = tail.trim_start_matches(' ');
            if trimmed.starts_with('=') || (trimmed.starts_with(':') && !trimmed.starts_with("://")) {
                let op = i + name.len() + skip + (tail.len() - trimmed.len()) + 1;
                consider(op);
            }
            from = i + name.len();
        }
    }
    best
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
        // A JWT's three parts are joined by dots.
        let jwt = rest[start..].starts_with("eyJ");
        let token_len = rest[start..]
            .chars()
            .take_while(|&c| is_key_char(c) || (jwt && c == '.'))
            .map(char::len_utf8)
            .sum::<usize>();
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
        assert_eq!(
            redact("sk-short and task-sk-0123456789abcdefghij", &[]),
            "sk-short and task-sk-0123456789abcdefghij"
        );
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

    /// The seeded corpus behind the brief's redaction proof (ROADMAP gap item 5): fake secrets in the shapes people
    /// paste into prompts and that tools print. Built by concatenation so no complete key shape sits in the source.
    pub(crate) fn seeded_secrets() -> Vec<(&'static str, String)> {
        let r = "Q7vX2mK9pL4nR8sT1wY6zB3cD5fG0hJ";
        vec![
            ("openai", format!("{}{}{}", "sk-", "proj-", r)),
            ("anthropic", format!("{}{}{}", "sk-ant-", "api03-", r)),
            ("openrouter", format!("{}{}{}", "sk-or-", "v1-", r)),
            ("github classic", format!("{}{}", "gh", "p_Ab3dEf6hIj9kLm2nOp5qRs8tUv1wXy4zAb7c")),
            (
                "github fine-grained",
                format!("{}{}{}", "github_", "pat_", "11ABCDEFG0123456789_abcdefghijklmnopqrstuvwxyz"),
            ),
            ("gitlab", format!("{}{}", "glp", "at-Ab3dEf6hIj9kLm2nOp5q")),
            ("aws access key id", format!("{}{}", "AK", "IAQ7VX2MK9PL4NR8ST")),
            ("aws secret", format!("aws_secret_access_key = {}", "wJalrXUtnFEMI/K7MDENG/bPxRfiCY1234567890")),
            ("google api key", format!("{}{}", "AI", "zaSyQ7vX2mK9pL4nR8sT1wY6zB3cD5fG0hJk")),
            ("slack bot", format!("{}{}", "xo", "xb-1234567890-0987654321-Q7vX2mK9pL4nR8sT")),
            ("stripe", format!("{}{}{}", "sk_", "live_", r)),
            ("huggingface", format!("{}{}", "h", "f_Q7vX2mK9pL4nR8sT1wY6zB3cD5fG0hJkLm")),
            ("npm", format!("{}{}", "np", "m_Q7vX2mK9pL4nR8sT1wY6zB3cD5fG0hJkLm")),
            (
                "jwt",
                format!(
                    "{}.{}.{}",
                    "eyJhbGciOiJIUzI1NiJ9", "eyJzdWIiOiIxMjM0NTY3ODkwIn0", "Q7vX2mK9pL4nR8sT1wY6zB3cD5fG0hJ"
                ),
            ),
            ("bearer header", format!("Authorization: Bearer {}", "Q7vX2mK9pL4nR8sT1wY6zB3cD5fG0hJ")),
            ("password assignment", "DB_PASSWORD=Tr0ub4dor-and-3-horses".to_string()),
            ("password in yaml", "password: \"c0rrect-h0rse-battery\"".to_string()),
            ("url credentials", "postgres://admin:S3cretPassw0rd@db.internal:5432/app".to_string()),
            (
                "private key",
                format!(
                    "-----BEGIN {} KEY-----\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASC\n-----END {} KEY-----",
                    "PRIVATE", "PRIVATE"
                ),
            ),
            (
                "azure connection string",
                format!("AccountKey={};EndpointSuffix=core.windows.net", "Q7vX2mK9pL4nR8sT1wY6zB3cD5fG0hJ+abc=="),
            ),
        ]
    }

    /// The secret part of a seeded line: what must not survive.
    pub(crate) fn secret_part(line: &str) -> &str {
        for sep in ["Bearer ", "= ", "=", ": \"", "admin:", "AccountKey="] {
            if let Some(i) = line.find(sep) {
                let rest = &line[i + sep.len()..];
                let end = rest.find(['@', '"', ';']).unwrap_or(rest.len());
                if end >= 8 {
                    return &rest[..end];
                }
            }
        }
        if line.starts_with("-----BEGIN") {
            return "MIIEvQIBADANBgkqhkiG9w0BAQEFAASC";
        }
        line
    }

    #[test]
    fn ordinary_text_is_left_alone() {
        for text in [
            "Author: Alexander Hamilton",
            "input_tokens: 123456789",
            "see https://example.com/docs?page=2",
            "password: none",
            "the token budget is large",
            "-----BEGIN CERTIFICATE-----",
        ] {
            assert_eq!(redact(text, &[]), text);
        }
    }

    #[test]
    fn the_seeded_secrets_are_masked() {
        let leaked: Vec<&str> = seeded_secrets()
            .iter()
            .filter(|(_, line)| redact(&format!("the value is {line} here"), &[]).contains(secret_part(line)))
            .map(|(name, _)| *name)
            .collect();
        assert!(leaked.is_empty(), "leaked: {leaked:?}");
    }
}
