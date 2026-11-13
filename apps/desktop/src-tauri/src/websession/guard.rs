//! The web guard (ADR-035, owner 2026-11-13): how much a web session may be asked, and when.
//!
//! DeepSeek suspended the owner's account on 2026-11-13 ("violation of user policies") after Pygg sent it 24 prompts of
//! 2,000 to 6,000 tokens in 15 minutes: one at a time with a 10 s rest, under the daily limit of 50. Counting requests
//! and resting a fixed time was enough for short questions and not for a stream of pasted files. The guard weighs each
//! request by its size, rests after a reply as long as a person would take to read it, caps what an account sends in an
//! hour and in a day by weight, refuses a prompt no person would type into a chat box, and backs off when the page shows
//! signs of trouble. It decides from what Pygg recorded (`web_pace`, `providers.web_*`), so it holds across restarts.
//! It slows Pygg down; it never disguises it (no random jitter, no rotation to get around a cap).

/// The longest prompt Pygg sends to a site, in characters (about 3,000 tokens): a long question with some pasted text.
/// Longer work, such as whole files, belongs on an API connection.
pub const MAX_PROMPT_CHARS: usize = 12_000;
/// A request weighs one unit, and one more for each full block of this many characters of prompt (1,999 characters weigh
/// 1, 2,000 weigh 2).
pub const CHARS_PER_UNIT: usize = 2_000;
/// What one account may send in any rolling hour: units, and requests whatever their size.
pub const HOUR_UNITS: u32 = 20;
pub const HOUR_REQUESTS: u32 = 15;
/// Units per UTC day: this many times the connection's daily request limit (100 with the default of 50).
pub const DAY_UNITS_PER_REQUEST: u32 = 2;
/// A request waits in line at most this long for its rest to end; beyond it, it is refused with a Retry-After.
pub const MAX_WAIT_MS: i64 = 30_000;
/// The rest after a reply: the base pace, two seconds per extra unit of prompt, and the time a person takes to read the
/// reply (50 characters a second), at most three minutes.
pub const REST_BASE_MS: i64 = super::PACE_SECS as i64 * 1000;
pub const REST_PER_UNIT_MS: i64 = 2_000;
pub const READ_MS_PER_CHAR: i64 = 20;
pub const REST_CAP_MS: i64 = 180_000;
/// After a sign of trouble (no reply in time, a usage-limit notice, a check, a changed page): 5 minutes, doubled for
/// each one in a row, at most 6 hours. A clean reply clears it.
pub const BACKOFF_BASE_MS: i64 = 5 * 60_000;
pub const BACKOFF_CAP_MS: i64 = 6 * 3_600_000;

pub const HOUR_MS: i64 = 3_600_000;
pub const DAY_MS: i64 = 86_400_000;

/// A request's weight: one unit, and one more per full 2,000 characters of prompt.
pub fn weight(prompt_chars: usize) -> u32 {
    1 + (prompt_chars / CHARS_PER_UNIT) as u32
}

/// The rest after a reply of `reply_chars` to a request of `weight`, from a base rest of `base_ms` (`REST_BASE_MS`).
pub fn rest_ms(base_ms: i64, weight: u32, reply_chars: usize) -> i64 {
    if base_ms == 0 {
        return 0; // tests
    }
    (base_ms + i64::from(weight.saturating_sub(1)) * REST_PER_UNIT_MS + reply_chars as i64 * READ_MS_PER_CHAR).min(REST_CAP_MS)
}

/// How long an account rests after its `strikes`-th sign of trouble in a row, from `base_ms` (`BACKOFF_BASE_MS`).
pub fn backoff_ms(base_ms: i64, strikes: u32) -> i64 {
    let doublings = strikes.saturating_sub(1).min(10);
    (base_ms << doublings).min(BACKOFF_CAP_MS)
}

/// What Pygg recorded about an account: the requests it sent (when, and their weight), and its rest and back-off.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Account {
    /// Requests sent in the last day, oldest first: (sent at, weight).
    pub sent: Vec<(i64, u32)>,
    /// When the rest after the last reply ends.
    pub next_at: Option<i64>,
    /// Backed off until then, after a sign of trouble, and why.
    pub backoff_until: Option<i64>,
    pub backoff_reason: Option<String>,
}

impl Account {
    /// Units and requests in the hour before `now`.
    pub fn hour(&self, now: i64) -> (u32, u32) {
        let recent = self.sent.iter().filter(|(at, _)| *at > now - HOUR_MS);
        recent.fold((0, 0), |(units, n), (_, w)| (units + w, n + 1))
    }

    /// Units since 00:00 UTC.
    pub fn today(&self, now: i64) -> u32 {
        let day_start = now - now.rem_euclid(DAY_MS);
        self.sent.iter().filter(|(at, _)| *at >= day_start).map(|(_, w)| w).sum()
    }
}

/// What the guard says about a request of `weight` at `now`.
#[derive(Clone, Debug, PartialEq)]
pub enum Verdict {
    /// Send it, after waiting this long (the end of the rest, at most `MAX_WAIT_MS`).
    Go { wait_ms: i64 },
    /// Not now: the earliest it could go, and Pygg's reason.
    Refuse { retry_after_ms: i64, why: &'static str },
}

pub const WHY_BACKOFF: &str = "Not sent: the site showed a sign of trouble on this account a moment ago (no reply in time, a usage-limit notice, a check or a changed page), so Pygg rests it before asking again.";
pub const WHY_HOUR: &str = "Not sent: this account has had its hour's share (20 units, a unit for each 2,000 characters of prompt, or 15 requests). Pygg keeps a web session at a person's pace.";
pub const WHY_DAY: &str = "Not sent: this account has had its day's share of long prompts (twice its daily request limit in units, a unit for each 2,000 characters). It resets at 00:00 UTC.";
pub const WHY_REST: &str = "Not sent yet: this account is resting after its last reply, as long as a person would take to read it.";

/// Whether a request of `weight` may go to this account at `now`, with its daily request limit `daily_limit`.
pub fn decide(account: &Account, weight: u32, now: i64, daily_limit: u32) -> Verdict {
    if let Some(until) = account.backoff_until.filter(|until| *until > now) {
        return Verdict::Refuse { retry_after_ms: until - now, why: WHY_BACKOFF };
    }
    let (units, requests) = account.hour(now);
    if units + weight > HOUR_UNITS || requests + 1 > HOUR_REQUESTS {
        // The earliest moment enough of the hour's requests have aged out for this one to fit.
        let mut free_units = HOUR_UNITS.saturating_sub(units);
        let mut free_requests = HOUR_REQUESTS.saturating_sub(requests);
        let mut at = now + HOUR_MS;
        for (sent, w) in account.sent.iter().filter(|(sent, _)| *sent > now - HOUR_MS) {
            free_units += w;
            free_requests += 1;
            if free_units >= weight && free_requests >= 1 {
                at = sent + HOUR_MS;
                break;
            }
        }
        return Verdict::Refuse { retry_after_ms: (at - now).max(1_000), why: WHY_HOUR };
    }
    if account.today(now) + weight > daily_limit.saturating_mul(DAY_UNITS_PER_REQUEST) {
        return Verdict::Refuse { retry_after_ms: (DAY_MS - now.rem_euclid(DAY_MS)).max(1_000), why: WHY_DAY };
    }
    let wait = account.next_at.map_or(0, |next| (next - now).max(0));
    if wait > MAX_WAIT_MS {
        return Verdict::Refuse { retry_after_ms: wait, why: WHY_REST };
    }
    Verdict::Go { wait_ms: wait }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_794_560_000_000;

    #[test]
    fn a_long_prompt_weighs_like_many_questions_and_rests_longer() {
        assert_eq!(weight(400), 1);
        assert_eq!(weight(2_000), 2);
        assert_eq!(weight(11_999), 6);
        // The benchmark's prompts: 8,000 to 24,000 characters.
        assert_eq!(weight(24_000), 13);
        assert_eq!(rest_ms(REST_BASE_MS, 1, 0), 10_000);
        assert_eq!(rest_ms(REST_BASE_MS, 1, 500), 20_000, "ten seconds to read 500 characters");
        assert_eq!(rest_ms(REST_BASE_MS, 6, 6_000), 140_000, "two minutes to read a 6,000-character module");
        assert_eq!(rest_ms(REST_BASE_MS, 6, 9_000), REST_CAP_MS, "never more than three minutes");
        assert_eq!(backoff_ms(BACKOFF_BASE_MS, 1), 5 * 60_000);
        assert_eq!(backoff_ms(BACKOFF_BASE_MS, 2), 10 * 60_000);
        assert_eq!(backoff_ms(BACKOFF_BASE_MS, 20), BACKOFF_CAP_MS);
    }

    #[test]
    fn the_run_that_got_deepseek_suspended_would_have_been_stopped() {
        // Prompts of 8,000 characters, one after another as soon as the rest allows.
        let mut account = Account::default();
        let mut now = NOW;
        let mut sent = 0;
        for _ in 0..24 {
            match decide(&account, weight(8_000), now, 50) {
                Verdict::Go { wait_ms } => {
                    now += wait_ms + 20_000;
                    account.sent.push((now, weight(8_000)));
                    account.next_at = Some(now + rest_ms(REST_BASE_MS, weight(8_000), 4_000));
                    sent += 1;
                }
                Verdict::Refuse { retry_after_ms, why } => {
                    assert!(why == WHY_HOUR || why == WHY_REST, "{why}");
                    now += retry_after_ms;
                }
            }
            if now > NOW + 15 * 60_000 {
                break;
            }
        }
        assert!(sent <= 4, "at most four such prompts in a quarter of an hour, not 24: {sent}");
    }

    #[test]
    fn short_questions_go_at_a_persons_pace() {
        let account = Account { next_at: Some(NOW + 5_000), ..Default::default() };
        assert_eq!(decide(&account, 1, NOW, 50), Verdict::Go { wait_ms: 5_000 });
        let resting = Account { next_at: Some(NOW + 90_000), ..Default::default() };
        assert_eq!(decide(&resting, 1, NOW, 50), Verdict::Refuse { retry_after_ms: 90_000, why: WHY_REST });

        // Fifteen questions in the hour: the sixteenth waits until the oldest is an hour old.
        let busy = Account { sent: (0..15).map(|i| (NOW - 50 * 60_000 + i * 60_000, 1)).collect(), ..Default::default() };
        assert_eq!(decide(&busy, 1, NOW, 50), Verdict::Refuse { retry_after_ms: 10 * 60_000, why: WHY_HOUR });
    }

    #[test]
    fn trouble_backs_off_and_the_day_has_a_share() {
        let troubled = Account { backoff_until: Some(NOW + 300_000), ..Default::default() };
        assert_eq!(decide(&troubled, 1, NOW, 50), Verdict::Refuse { retry_after_ms: 300_000, why: WHY_BACKOFF });

        let day_start = NOW - NOW.rem_euclid(DAY_MS);
        // 96 units today, spread over earlier hours: a 5-unit prompt passes the day's 100.
        let full = Account { sent: (0..16).map(|i| (day_start + i * 1_000, 6)).collect(), ..Default::default() };
        assert!(matches!(decide(&full, 5, NOW.max(day_start + 2 * HOUR_MS), 50), Verdict::Refuse { why: WHY_DAY, .. }));
        assert!(matches!(decide(&full, 4, NOW.max(day_start + 2 * HOUR_MS), 50), Verdict::Go { .. }));
    }
}
