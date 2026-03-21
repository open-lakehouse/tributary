//! Human-friendly time parsing for `--start-time` / `--end-time` CLI arguments.
//!
//! Accepted formats:
//!
//! | Input                 | Meaning                                      |
//! |-----------------------|----------------------------------------------|
//! | `now`                 | Current UTC time                             |
//! | `now+3h`, `now+30m`   | Relative positive offset from now            |
//! | `now-1h`, `now-30m`   | Relative negative offset from now            |
//! | `2026-01-01`          | Midnight UTC on that date                    |
//! | `2026-01-01T08:00:00Z`| Full RFC 3339 datetime                       |

use chrono::{DateTime, NaiveDate, TimeZone, Utc};

/// Parse a human-friendly time string into a `DateTime<Utc>`.
///
/// See module-level docs for accepted formats.
pub fn parse_sim_time(s: &str) -> Result<DateTime<Utc>, String> {
    let s = s.trim();

    // "now" → current time
    if s == "now" {
        return Ok(Utc::now());
    }

    // "now+..." or "now-..." → relative offset
    if let Some(rest) = s.strip_prefix("now+") {
        let duration = humantime::parse_duration(rest)
            .map_err(|e| format!("invalid duration '{}': {}", rest, e))?;
        let nanos = duration.as_nanos() as i64;
        return Ok(Utc::now() + chrono::Duration::nanoseconds(nanos));
    }
    if let Some(rest) = s.strip_prefix("now-") {
        let duration = humantime::parse_duration(rest)
            .map_err(|e| format!("invalid duration '{}': {}", rest, e))?;
        let nanos = duration.as_nanos() as i64;
        return Ok(Utc::now() - chrono::Duration::nanoseconds(nanos));
    }

    // Date-only: "2026-01-01" → midnight UTC
    if let Ok(date) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        let midnight = date.and_hms_opt(0, 0, 0).expect("midnight is always valid");
        return Utc
            .from_local_datetime(&midnight)
            .single()
            .ok_or_else(|| format!("ambiguous date '{}'", s));
    }

    // RFC 3339 fallback
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| {
            format!(
                "cannot parse '{}' as a time (try: now, now+1h, 2026-01-01, or RFC 3339): {}",
                s, e
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_now() {
        let before = Utc::now();
        let t = parse_sim_time("now").unwrap();
        let after = Utc::now();
        assert!(t >= before && t <= after);
    }

    #[test]
    fn test_now_plus_offset() {
        let t = parse_sim_time("now+1h").unwrap();
        let now = Utc::now();
        let diff = (t - now).num_seconds();
        assert!((3500..=3700).contains(&diff));
    }

    #[test]
    fn test_now_minus_offset() {
        let t = parse_sim_time("now-30m").unwrap();
        let now = Utc::now();
        let diff = (now - t).num_seconds();
        assert!((1700..=1900).contains(&diff));
    }

    #[test]
    fn test_date_only() {
        let t = parse_sim_time("2026-01-01").unwrap();
        assert_eq!(
            t.format("%Y-%m-%d %H:%M:%S").to_string(),
            "2026-01-01 00:00:00"
        );
    }

    #[test]
    fn test_rfc3339() {
        let t = parse_sim_time("2026-06-01T12:00:00Z").unwrap();
        assert_eq!(
            t.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            "2026-06-01T12:00:00Z"
        );
    }

    #[test]
    fn test_invalid() {
        assert!(parse_sim_time("garbage").is_err());
    }
}
