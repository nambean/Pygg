//! The web guard (ADR-035, owner 2026-11-13): how much a web session may be asked, and when.
//!
//! DeepSeek suspended the owner's account on 2026-11-13 ("violation of user policies") after Pygg sent it 24 prompts of
//! 2,000 to 6,000 tokens in 15 minutes: one at a time with a 10 s rest, under the daily limit of 50. Counting requests
//! and resting a fixed time was enough for short questions and not for a stream of pasted files. The guard weighs each
//! request by its size, rests after a reply as long as a person would take to read it, caps what an account sends in an
//! hour and in a day by weight, refuses a prompt no person would type into a chat box, and backs off when the page shows
//! signs of trouble. It decides from what Pygg recorded (`web_pace`, `providers.web_*`), so it holds across restarts.
//! It slows Pygg down; it never disguises it (no random jitter, no rotation to get around a cap).

/// The longest prompt Pygg sends to a site by default, in characters (about 3,000 tokens): a long question with some
/// pasted text. Longer work, such as whole files, belongs on an API connection. A site can have a lower one (`limits`).
pub const MAX_PROMPT_CHARS: usize = 12_000;
/// A request weighs one unit, and one more for each full block of this many characters of prompt (1,999 characters weigh
/// 1, 2,000 weigh 2).
pub const CHARS_PER_UNIT: usize = 2_000;
/// What one account may send in any rolling hour by default: units, and requests whatever their size.
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
/// each one in a row, at most 6 hours, and never less than the kind of trouble asks (`Trouble`). A clean reply clears it.
pub const BACKOFF_BASE_MS: i64 = 5 * 60_000;
pub const BACKOFF_CAP_MS: i64 = 6 * 3_600_000;
/// No burst: at most this many requests in any 10 minutes by default, however short. A person asks, reads, thinks.
pub const BURST_REQUESTS: u32 = 5;
pub const BURST_WINDOW_MS: i64 = 10 * 60_000;
/// The highest daily request limit a user may set for a site by default (they may set less).
pub const MAX_DAILY_LIMIT: u32 = 100;

pub const HOUR_MS: i64 = 3_600_000;
pub const DAY_MS: i64 = 86_400_000;

/// One site's limits. Every number is a ceiling the user cannot raise from the app: the daily request limit they set is
/// held under `max_daily_limit`, and nothing else is a setting. A site that has suspended an account for Pygg's pace
/// gets lower ones (DeepSeek, 2026-11-13).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    pub max_prompt_chars: usize,
    pub hour_units: u32,
    pub hour_requests: u32,
    pub burst_requests: u32,
    pub burst_window_ms: i64,
    pub max_daily_limit: u32,
    /// The rest after a reply starts from at least this (the base pace is 10 s).
    pub rest_base_ms: i64,
    /// Why the site's limits are lower than the default, for Providers.
    pub note: Option<&'static str>,
}

pub const DEFAULT_LIMITS: Limits = Limits { max_prompt_chars: MAX_PROMPT_CHARS, hour_units: HOUR_UNITS,
    hour_requests: HOUR_REQUESTS, burst_requests: BURST_REQUESTS, burst_window_ms: BURST_WINDOW_MS,
    max_daily_limit: MAX_DAILY_LIMIT, rest_base_ms: REST_BASE_MS, note: None };

/// DeepSeek suspended the owner's account after 24 prompts of 2,000 to 6,000 tokens in 15 minutes. Here a prompt is at
/// most 4,000 characters (about 1,000 tokens), an hour holds 8 units and 6 requests, 10 minutes 3 requests, a day at
/// most 30 requests, and the rest after a reply starts at 30 s.
pub const DEEPSEEK_LIMITS: Limits = Limits { max_prompt_chars: 4_000, hour_units: 8, hour_requests: 6, burst_requests: 3,
    burst_window_ms: BURST_WINDOW_MS, max_daily_limit: 30, rest_base_ms: 30_000,
    note: Some("DeepSeek suspended an account for Pygg's pace on 2026-11-13, so its limits are lower than other sites'.") };

/// The limits for a web-session preset (`deepseek-web`, `muse-web`, ...).
pub fn limits(preset: &str) -> Limits {
    match preset {
        "deepseek-web" => DEEPSEEK_LIMITS,
        _ => DEFAULT_LIMITS,
    }
}

impl Limits {
    /// The daily request limit in force: the user's, held under the site's ceiling.
    pub fn daily_limit(&self, configured: u32) -> u32 {
        configured.clamp(1, self.max_daily_limit)
    }

    /// The base of the rest after a reply, from the service's pace `gap_ms` (zero only in tests).
    pub fn rest_base(&self, gap_ms: i64) -> i64 {
        if gap_ms == 0 { 0 } else { gap_ms.max(self.rest_base_ms) }
    }
}

/// A sign of trouble, and the least it rests the account: a usage-limit notice an hour, a site asking to slow down or
/// showing a check half an hour, a suspension notice six hours, anything else (no reply in time, a changed page) the
/// first step of the back-off. The doubling of the back-off applies on top.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trouble {
    pub reason: &'static str,
    /// The least back-off, in multiples of the base back-off (5 minutes).
    pub floor: u32,
}

impl Trouble {
    pub const fn new(reason: &'static str, floor: u32) -> Trouble {
        Trouble { reason, floor }
    }

    /// The back-off for the `strikes`-th sign of trouble in a row, from `base_ms` (`BACKOFF_BASE_MS`).
    pub fn backoff_ms(&self, base_ms: i64, strikes: u32) -> i64 {
        backoff_ms(base_ms, strikes).max(base_ms.saturating_mul(i64::from(self.floor))).min(BACKOFF_CAP_MS)
    }
}

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

    /// Requests in the `window_ms` before `now`.
    pub fn burst(&self, now: i64, window_ms: i64) -> u32 {
        self.sent.iter().filter(|(at, _)| *at > now - window_ms).count() as u32
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

pub const WHY_BACKOFF: &str = "Not sent: the site showed a sign of trouble on this account a moment ago (no reply in time, a usage-limit notice, a request to slow down, a check or a changed page), so Pygg rests it before asking again.";
pub const WHY_HOUR: &str = "Not sent: this account has had its hour's share (units by prompt size, and requests; Providers shows this site's numbers). Pygg keeps a web session at a person's pace.";
pub const WHY_BURST: &str = "Not sent: this account has had its share of the last 10 minutes. A person asks, reads and thinks between questions; Pygg does not send bursts.";
pub const WHY_DAY: &str = "Not sent: this account has had its day's share of long prompts (twice its daily request limit in units, a unit for each 2,000 characters). It resets at 00:00 UTC.";
pub const WHY_REST: &str = "Not sent yet: this account is resting after its last reply, as long as a person would take to read it.";

/// The earliest moment, after `now`, by which enough of the requests sent after `since` have aged out of a window of
/// `window_ms` to leave `free_units` units and `free_requests` requests (starting from what is free now).
fn aged_out(account: &Account, now: i64, window_ms: i64, mut free_units: u32, mut free_requests: u32, weight: u32) -> i64 {
    for (sent, w) in account.sent.iter().filter(|(sent, _)| *sent > now - window_ms) {
        free_units += w;
        free_requests += 1;
        if free_units >= weight && free_requests >= 1 {
            return sent + window_ms;
        }
    }
    now + window_ms
}

/// Whether a request of `weight` may go to this account at `now`, with its daily request limit `daily_limit` (already
/// held under the site's ceiling) and the site's `limits`.
pub fn decide(account: &Account, weight: u32, now: i64, daily_limit: u32, limits: &Limits) -> Verdict {
    if let Some(until) = account.backoff_until.filter(|until| *until > now) {
        return Verdict::Refuse { retry_after_ms: until - now, why: WHY_BACKOFF };
    }
    let (units, requests) = account.hour(now);
    if units + weight > limits.hour_units || requests + 1 > limits.hour_requests {
        let at = aged_out(account, now, HOUR_MS, limits.hour_units.saturating_sub(units),
            limits.hour_requests.saturating_sub(requests), weight);
        return Verdict::Refuse { retry_after_ms: (at - now).max(1_000), why: WHY_HOUR };
    }
    let recent = account.burst(now, limits.burst_window_ms);
    if recent + 1 > limits.burst_requests {
        let at = aged_out(account, now, limits.burst_window_ms, u32::MAX / 2, limits.burst_requests.saturating_sub(recent), 0);
        return Verdict::Refuse { retry_after_ms: (at - now).max(1_000), why: WHY_BURST };
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
            match decide(&account, weight(8_000), now, 50, &DEFAULT_LIMITS) {
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
        assert_eq!(decide(&account, 1, NOW, 50, &DEFAULT_LIMITS), Verdict::Go { wait_ms: 5_000 });
        let resting = Account { next_at: Some(NOW + 90_000), ..Default::default() };
        assert_eq!(decide(&resting, 1, NOW, 50, &DEFAULT_LIMITS), Verdict::Refuse { retry_after_ms: 90_000, why: WHY_REST });

        // Fifteen questions in the hour: the sixteenth waits until the oldest is an hour old.
        let busy = Account { sent: (0..15).map(|i| (NOW - 50 * 60_000 + i * 60_000, 1)).collect(), ..Default::default() };
        assert_eq!(decide(&busy, 1, NOW, 50, &DEFAULT_LIMITS), Verdict::Refuse { retry_after_ms: 10 * 60_000, why: WHY_HOUR });
    }

    #[test]
    fn trouble_backs_off_and_the_day_has_a_share() {
        let troubled = Account { backoff_until: Some(NOW + 300_000), ..Default::default() };
        assert_eq!(decide(&troubled, 1, NOW, 50, &DEFAULT_LIMITS), Verdict::Refuse { retry_after_ms: 300_000, why: WHY_BACKOFF });

        let day_start = NOW - NOW.rem_euclid(DAY_MS);
        // 96 units today, spread over earlier hours: a 5-unit prompt passes the day's 100.
        let full = Account { sent: (0..16).map(|i| (day_start + i * 1_000, 6)).collect(), ..Default::default() };
        assert!(matches!(decide(&full, 5, NOW.max(day_start + 2 * HOUR_MS), 50, &DEFAULT_LIMITS), Verdict::Refuse { why: WHY_DAY, .. }));
        assert!(matches!(decide(&full, 4, NOW.max(day_start + 2 * HOUR_MS), 50, &DEFAULT_LIMITS), Verdict::Go { .. }));
    }

    #[test]
    fn short_questions_do_not_go_in_bursts() {
        // Five quick questions in the last ten minutes: the sixth waits until the oldest is ten minutes old.
        let quick = Account { sent: (0..5).map(|i| (NOW - 8 * 60_000 + i * 60_000, 1)).collect(), ..Default::default() };
        assert_eq!(decide(&quick, 1, NOW, 50, &DEFAULT_LIMITS), Verdict::Refuse { retry_after_ms: 2 * 60_000, why: WHY_BURST });
        let four = Account { sent: quick.sent[1..].to_vec(), ..Default::default() };
        assert_eq!(decide(&four, 1, NOW, 50, &DEFAULT_LIMITS), Verdict::Go { wait_ms: 0 });
        // DeepSeek takes three in ten minutes.
        let three = Account { sent: quick.sent[2..].to_vec(), ..Default::default() };
        assert!(matches!(decide(&three, 1, NOW, 30, &DEEPSEEK_LIMITS), Verdict::Refuse { why: WHY_BURST, .. }));
    }

    #[test]
    fn deepseek_has_lower_limits_than_any_other_site() {
        let d = limits("deepseek-web");
        assert_eq!(d, DEEPSEEK_LIMITS);
        for preset in super::super::RECIPES.iter().map(|r| r.preset) {
            let l = limits(preset);
            assert!(d.max_prompt_chars <= l.max_prompt_chars && d.hour_units <= l.hour_units
                && d.hour_requests <= l.hour_requests && d.burst_requests <= l.burst_requests
                && d.max_daily_limit <= l.max_daily_limit && d.rest_base_ms >= l.rest_base_ms, "{preset}");
            assert!(l.max_prompt_chars <= MAX_PROMPT_CHARS && l.max_daily_limit <= MAX_DAILY_LIMIT, "no site above the default");
        }
        // The benchmark's prompts (8,000 to 24,000 characters) are over DeepSeek's limit before anything is weighed.
        assert!(8_000 > d.max_prompt_chars);
        // The daily limit a user sets is held under the site's ceiling, and never 0.
        assert_eq!(d.daily_limit(1_000), 30);
        assert_eq!(d.daily_limit(0), 1);
        assert_eq!(DEFAULT_LIMITS.daily_limit(1_000), MAX_DAILY_LIMIT);
        assert_eq!(DEFAULT_LIMITS.daily_limit(20), 20);
        // The rest starts at 30 s on DeepSeek, 10 s elsewhere; zero only when the service's pace is zero (tests).
        assert_eq!(d.rest_base(REST_BASE_MS), 30_000);
        assert_eq!(DEFAULT_LIMITS.rest_base(REST_BASE_MS), REST_BASE_MS);
        assert_eq!(d.rest_base(0), 0);
    }

    #[test]
    fn the_deepseek_run_sends_at_most_four_short_prompts_in_a_quarter_hour() {
        // The run that got the account suspended, cut to DeepSeek's prompt limit: 3,900 characters each.
        let mut account = Account::default();
        let mut now = NOW;
        let mut sent = 0;
        let l = DEEPSEEK_LIMITS;
        while now < NOW + 15 * 60_000 {
            match decide(&account, weight(3_900), now, l.daily_limit(50), &l) {
                Verdict::Go { wait_ms } => {
                    now += wait_ms + 20_000;
                    account.sent.push((now, weight(3_900)));
                    account.next_at = Some(now + rest_ms(l.rest_base(REST_BASE_MS), weight(3_900), 2_000));
                    sent += 1;
                }
                Verdict::Refuse { retry_after_ms, .. } => now += retry_after_ms,
            }
        }
        // Three in the first ten minutes, a fourth once the first has aged out: about 4,000 tokens, not 24 x 4,000.
        assert!(sent <= 4, "{sent}");
        assert!(account.hour(now).0 <= l.hour_units);
    }

    #[test]
    fn a_sites_own_warning_rests_the_account_longer_than_a_slow_page() {
        let slow = Trouble::new("slow", 1);
        let quota = Trouble::new("quota", 12);
        let restricted = Trouble::new("restricted", 72);
        assert_eq!(slow.backoff_ms(BACKOFF_BASE_MS, 1), 5 * 60_000);
        assert_eq!(quota.backoff_ms(BACKOFF_BASE_MS, 1), HOUR_MS, "a usage-limit notice: an hour at least");
        assert_eq!(quota.backoff_ms(BACKOFF_BASE_MS, 6), 160 * 60_000, "the doubling goes on above the floor");
        assert_eq!(restricted.backoff_ms(BACKOFF_BASE_MS, 1), BACKOFF_CAP_MS);
        assert_eq!(quota.backoff_ms(0, 3), 0, "only a zero pace (tests) rests nothing");
    }
}
