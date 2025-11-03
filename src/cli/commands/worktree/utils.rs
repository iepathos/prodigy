//! Utility functions for worktree command

use anyhow::Result;
use std::time::Duration;

/// Parse duration string (e.g., "1ms", "5s", "10m", "2h", "7d")
///
/// # Examples
///
/// ```
/// use std::time::Duration;
///
/// let duration = parse_duration("1h").unwrap();
/// assert_eq!(duration, Duration::from_secs(3600));
///
/// let duration = parse_duration("24h").unwrap();
/// assert_eq!(duration, Duration::from_secs(86400));
///
/// let duration = parse_duration("7d").unwrap();
/// assert_eq!(duration, Duration::from_secs(604800));
/// ```
pub fn parse_duration(s: &str) -> Result<Duration> {
    let s = s.trim().to_lowercase();
    let (num_str, unit) = if s.ends_with("ms") {
        (&s[..s.len() - 2], "ms")
    } else if s.ends_with('s') {
        (&s[..s.len() - 1], "s")
    } else if s.ends_with('m') {
        (&s[..s.len() - 1], "m")
    } else if s.ends_with('h') {
        (&s[..s.len() - 1], "h")
    } else if s.ends_with('d') {
        (&s[..s.len() - 1], "d")
    } else {
        return Err(anyhow::anyhow!(
            "Invalid duration format. Use format like '1h', '24h', '7d'"
        ));
    };

    let num: u64 = num_str
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid number in duration"))?;

    Ok(match unit {
        "ms" => Duration::from_millis(num),
        "s" => Duration::from_secs(num),
        "m" => Duration::from_secs(num * 60),
        "h" => Duration::from_secs(num * 3600),
        "d" => Duration::from_secs(num * 86400),
        _ => unreachable!(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_duration_milliseconds() {
        let duration = parse_duration("100ms").unwrap();
        assert_eq!(duration, Duration::from_millis(100));
    }

    #[test]
    fn test_parse_duration_seconds() {
        let duration = parse_duration("30s").unwrap();
        assert_eq!(duration, Duration::from_secs(30));
    }

    #[test]
    fn test_parse_duration_minutes() {
        let duration = parse_duration("10m").unwrap();
        assert_eq!(duration, Duration::from_secs(600));
    }

    #[test]
    fn test_parse_duration_hours() {
        let duration = parse_duration("2h").unwrap();
        assert_eq!(duration, Duration::from_secs(7200));
    }

    #[test]
    fn test_parse_duration_days() {
        let duration = parse_duration("7d").unwrap();
        assert_eq!(duration, Duration::from_secs(604800));
    }

    #[test]
    fn test_parse_duration_case_insensitive() {
        let duration = parse_duration("5H").unwrap();
        assert_eq!(duration, Duration::from_secs(18000));
    }

    #[test]
    fn test_parse_duration_with_whitespace() {
        let duration = parse_duration("  3m  ").unwrap();
        assert_eq!(duration, Duration::from_secs(180));
    }

    #[test]
    fn test_parse_duration_invalid_format() {
        let result = parse_duration("10");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid duration format"));
    }

    #[test]
    fn test_parse_duration_invalid_number() {
        let result = parse_duration("abch");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid number in duration"));
    }

    #[test]
    fn test_parse_duration_empty_string() {
        let result = parse_duration("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_duration_only_unit() {
        let result = parse_duration("h");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_duration_large_values() {
        let duration = parse_duration("365d").unwrap();
        assert_eq!(duration, Duration::from_secs(31536000)); // 365 days in seconds
    }
}
