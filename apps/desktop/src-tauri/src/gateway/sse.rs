//! Watches an OpenAI-style server-sent event stream as it passes through, for the `usage` object that
//! providers send in the last chunk when asked with `stream_options.include_usage`. The bytes themselves
//! are forwarded unchanged.

use super::pricing::Usage;

/// The longest single event line kept; longer lines are skipped rather than buffered without bound.
const MAX_LINE: usize = 4 * 1024 * 1024;

#[derive(Default)]
pub struct UsageScanner {
    line: Vec<u8>,
    overflow: bool,
    pub usage: Option<Usage>,
    pub done: bool,
    pub chunks: usize,
}

impl UsageScanner {
    pub fn feed(&mut self, bytes: &[u8]) {
        for &b in bytes {
            if b == b'\n' {
                if !self.overflow {
                    let line = std::mem::take(&mut self.line);
                    self.line_done(&line);
                }
                self.line.clear();
                self.overflow = false;
            } else if !self.overflow {
                if self.line.len() >= MAX_LINE {
                    self.overflow = true;
                    self.line.clear();
                } else {
                    self.line.push(b);
                }
            }
        }
    }

    fn line_done(&mut self, line: &[u8]) {
        let Ok(text) = std::str::from_utf8(line) else { return };
        let Some(data) = text.trim_end_matches('\r').strip_prefix("data:") else { return };
        let data = data.trim();
        if data == "[DONE]" {
            self.done = true;
            return;
        }
        let Ok(event) = serde_json::from_str::<serde_json::Value>(data) else { return };
        self.chunks += 1;
        if let Some(usage) = Usage::from_json(&event["usage"]) {
            self.usage = Some(usage);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_usage_across_chunk_boundaries() {
        let stream = "data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}\r\n\r\n\
            data: {\"choices\":[],\"usage\":{\"prompt_tokens\":12,\"completion_tokens\":3}}\n\ndata: [DONE]\n\n";
        let mut scanner = UsageScanner::default();
        for piece in stream.as_bytes().chunks(7) {
            scanner.feed(piece);
        }
        assert!(scanner.done);
        assert_eq!(scanner.chunks, 2);
        let usage = scanner.usage.unwrap();
        assert_eq!((usage.prompt, usage.completion), (12, 3));
    }

    #[test]
    fn null_usage_is_not_usage() {
        let mut scanner = UsageScanner::default();
        scanner.feed(b"data: {\"usage\":null}\n");
        assert!(scanner.usage.is_none());
    }
}
