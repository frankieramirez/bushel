use std::time::{SystemTime, UNIX_EPOCH};

/// Seconds since the Unix epoch for an RFC 3339 timestamp.
///
/// Hand-rolled because bushel carries no date crate: the `container` CLI emits
/// `2026-08-25T00:59:14.123Z` (and the `+01:00` spelling), which is the whole
/// grammar we need.
pub fn epoch_secs(ts: &str) -> Option<i64> {
    let bytes = ts.as_bytes();
    if bytes.len() < 20 {
        return None;
    }
    if bytes[4] != b'-' || bytes[7] != b'-' || bytes[13] != b':' || bytes[16] != b':' {
        return None;
    }
    if bytes[10] != b'T' && bytes[10] != b't' {
        return None;
    }
    let (year, month, day) = (digits(ts, 0..4)?, digits(ts, 5..7)?, digits(ts, 8..10)?);
    let (hour, min, sec) = (
        digits(ts, 11..13)?,
        digits(ts, 14..16)?,
        digits(ts, 17..19)?,
    );
    if !(1..=12).contains(&month) || !(1..=days_in_month(year, month)).contains(&day) {
        return None;
    }
    if hour > 23 || min > 59 || sec > 60 {
        return None;
    }
    let secs = days_from_civil(year, month, day) * 86_400 + hour * 3_600 + min * 60 + sec;
    Some(secs - zone_offset(&ts[19..])?)
}

/// An all-digit field parsed as a number, or `None` for anything else.
fn digits(s: &str, range: std::ops::Range<usize>) -> Option<i64> {
    let field = s.get(range)?;
    if field.bytes().all(|b| b.is_ascii_digit()) {
        field.parse().ok()
    } else {
        None
    }
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Seconds to subtract for the zone closing a timestamp: `Z` or `±HH:MM`, after
/// the optional fractional seconds. `None` for a missing zone or trailing junk.
fn zone_offset(rest: &str) -> Option<i64> {
    let zone = match rest.strip_prefix('.') {
        Some(frac) => {
            let taken = frac.len() - frac.trim_start_matches(|c: char| c.is_ascii_digit()).len();
            if taken == 0 {
                return None;
            }
            &frac[taken..]
        }
        None => rest,
    };
    if zone == "Z" || zone == "z" {
        return Some(0);
    }
    let bytes = zone.as_bytes();
    if bytes.len() != 6 || bytes[3] != b':' {
        return None;
    }
    let (oh, om) = (digits(zone, 1..3)?, digits(zone, 4..6)?);
    if oh > 23 || om > 59 {
        return None;
    }
    let delta = oh * 3_600 + om * 60;
    match bytes[0] {
        b'+' => Some(delta),
        b'-' => Some(-delta),
        _ => None,
    }
}

/// Days between 1970-01-01 and y-m-d (Howard Hinnant's `days_from_civil`).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// `3h 12m`, `1d 4h`, `47s` — the two coarsest units that carry information.
pub fn short_duration(secs: i64) -> String {
    let secs = secs.max(0);
    let (d, h, m) = (secs / 86_400, secs % 86_400 / 3_600, secs % 3_600 / 60);
    if d > 0 {
        format!("{d}d {h}h")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else if m > 0 {
        format!("{m}m {}s", secs % 60)
    } else {
        format!("{secs}s")
    }
}

pub fn uptime(started: Option<&str>, now: i64) -> Option<String> {
    let at = epoch_secs(started?)?;
    Some(short_duration(now - at))
}

pub fn uptime_now(started: Option<&str>) -> Option<String> {
    uptime(started, now_secs())
}

/// `3h ago`, `10d ago` — the same clock read backwards, for `created`.
pub fn age(created: Option<&str>, now: i64) -> Option<String> {
    let at = epoch_secs(created?)?;
    Some(format!("{} ago", short_duration(now - at)))
}

pub fn age_now(created: Option<&str>) -> Option<String> {
    age(created, now_secs())
}

fn registry_token(host: &str) -> String {
    match host {
        "docker.io" | "index.docker.io" | "registry-1.docker.io" => "dh".into(),
        "ghcr.io" => "gh".into(),
        "quay.io" => "qy".into(),
        "gcr.io" => "gc".into(),
        "registry.k8s.io" | "k8s.gcr.io" => "k8".into(),
        "mcr.microsoft.com" => "ms".into(),
        "public.ecr.aws" => "ec".into(),
        other => {
            let mut token: String = other
                .chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .take(2)
                .collect();
            if token.is_empty() {
                token.push('?');
            }
            token.to_lowercase()
        }
    }
}

/// Split `docker.io/library/python:3.12-slim` into (`dh`, `python:3.12-slim`).
///
/// A reference with no registry host keeps an empty token, so `comicarr:latest`
/// still reads as itself. `library/` goes with the host it belongs to: on Docker
/// Hub it is noise, anywhere else it is a real namespace.
pub fn collapse_reference(reference: &str) -> (String, String) {
    let Some(slash) = reference.find('/') else {
        return (String::new(), reference.to_string());
    };
    let (host, rest) = reference.split_at(slash);
    let rest = &rest[1..];
    let is_host = host == "localhost" || host.contains('.') || host.contains(':');
    if !is_host {
        return (String::new(), reference.to_string());
    }
    let token = registry_token(host);
    let rest = if token == "dh" {
        rest.strip_prefix("library/").unwrap_or(rest)
    } else {
        rest
    };
    (token, rest.to_string())
}

/// Middle-elide a cell so both ends survive: `blakeblackshear/frig…:stable`.
pub fn elide(text: &str, width: usize) -> String {
    let n = text.chars().count();
    if n <= width {
        return text.to_string();
    }
    if width <= 1 {
        return "…".chars().take(width).collect();
    }
    let keep = width - 1;
    let head = keep.div_ceil(2);
    let tail = keep - head;
    let chars: Vec<char> = text.chars().collect();
    let mut out: String = chars[..head].iter().collect();
    out.push('…');
    out.extend(&chars[n - tail..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_parses_the_shapes_the_cli_emits() {
        assert_eq!(epoch_secs("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(epoch_secs("2026-08-25T00:59:14Z"), Some(1_787_619_554));
        assert_eq!(
            epoch_secs("2026-08-25T00:59:14.128Z"),
            Some(1_787_619_554),
            "fractional seconds are dropped, not fumbled"
        );
    }

    #[test]
    fn a_zone_offset_moves_the_instant() {
        let utc = epoch_secs("2026-08-25T00:00:00Z").unwrap();
        assert_eq!(epoch_secs("2026-08-25T01:00:00+01:00"), Some(utc));
        assert_eq!(epoch_secs("2026-08-24T23:00:00-01:00"), Some(utc));
    }

    #[test]
    fn garbage_timestamps_are_none_not_zero() {
        for bad in ["", "yesterday", "2026-08-25", "2026-13-01T00:00:00Z"] {
            assert_eq!(epoch_secs(bad), None, "{bad:?} should not parse");
        }
    }

    #[test]
    fn a_timestamp_without_a_zone_is_not_silently_utc() {
        assert_eq!(epoch_secs("2026-08-25T00:00:00"), None);
        assert_eq!(epoch_secs("2026-08-25T00:00:00.128"), None);
        assert_eq!(epoch_secs("2026-08-25T00:00:00+01"), None);
    }

    #[test]
    fn the_separators_are_fixed() {
        for bad in [
            "2026-08-25 00:00:00Z",
            "2026/08/25T00:00:00Z",
            "2026-08-25T00-00-00Z",
        ] {
            assert_eq!(epoch_secs(bad), None, "{bad:?} should not parse");
        }
        assert_eq!(
            epoch_secs("2026-08-25t00:59:14z"),
            Some(1_787_619_554),
            "the lowercase spellings are still RFC 3339"
        );
    }

    #[test]
    fn out_of_range_times_are_rejected_but_a_leap_second_is_not() {
        assert_eq!(epoch_secs("2026-08-25T24:00:00Z"), None);
        assert_eq!(epoch_secs("2026-08-25T00:60:00Z"), None);
        assert_eq!(
            epoch_secs("2026-08-25T23:59:60Z"),
            Some(1_787_702_400),
            "RFC 3339 permits second 60"
        );
    }

    #[test]
    fn the_day_is_checked_against_the_real_month() {
        assert_eq!(epoch_secs("2026-02-29T00:00:00Z"), None);
        assert_eq!(epoch_secs("2026-02-30T00:00:00Z"), None);
        assert_eq!(epoch_secs("2026-04-31T00:00:00Z"), None);
        assert_eq!(epoch_secs("1900-02-29T00:00:00Z"), None);
        assert!(epoch_secs("2024-02-29T00:00:00Z").is_some());
        assert!(epoch_secs("2000-02-29T00:00:00Z").is_some());
    }

    #[test]
    fn month_and_day_zero_are_not_dates() {
        assert_eq!(epoch_secs("2026-00-01T00:00:00Z"), None);
        assert_eq!(epoch_secs("2026-08-00T00:00:00Z"), None);
    }

    #[test]
    fn a_malformed_zone_is_rejected() {
        for bad in [
            "2026-08-25T00:00:00+24:00",
            "2026-08-25T00:00:00+00:60",
            "2026-08-25T00:00:00+0100",
            "2026-08-25T00:00:00.Z",
            "2026-08-25T00:00:00Zulu",
            "2026-08-25T00:00:00.128Z ",
        ] {
            assert_eq!(epoch_secs(bad), None, "{bad:?} should not parse");
        }
    }

    #[test]
    fn durations_show_the_two_coarsest_units() {
        assert_eq!(short_duration(47), "47s");
        assert_eq!(short_duration(9 * 60 + 3), "9m 3s");
        assert_eq!(short_duration(3 * 3600 + 12 * 60), "3h 12m");
        assert_eq!(short_duration(28 * 3600), "1d 4h");
        assert_eq!(
            short_duration(-5),
            "0s",
            "a clock skew is not a negative age"
        );
    }

    #[test]
    fn uptime_and_age_read_the_same_clock_two_ways() {
        let now = epoch_secs("2026-08-25T03:12:00Z").unwrap();
        assert_eq!(
            uptime(Some("2026-08-25T00:00:00Z"), now).as_deref(),
            Some("3h 12m")
        );
        assert_eq!(
            age(Some("2026-08-25T00:00:00Z"), now).as_deref(),
            Some("3h 12m ago")
        );
        assert_eq!(uptime(None, now), None);
        assert_eq!(age(Some("nonsense"), now), None);
    }

    #[test]
    fn known_registries_collapse_to_two_letters() {
        assert_eq!(
            collapse_reference("docker.io/library/python:3.12-slim"),
            ("dh".into(), "python:3.12-slim".into())
        );
        assert_eq!(
            collapse_reference("ghcr.io/astral-sh/uv:0.10.4"),
            ("gh".into(), "astral-sh/uv:0.10.4".into())
        );
        assert_eq!(
            collapse_reference("quay.io/prometheus/node-exporter:v1"),
            ("qy".into(), "prometheus/node-exporter:v1".into())
        );
    }

    #[test]
    fn an_unknown_registry_borrows_its_first_two_letters() {
        assert_eq!(
            collapse_reference("registry.example.com/team/app:1"),
            ("re".into(), "team/app:1".into())
        );
        assert_eq!(
            collapse_reference("localhost:5000/dev/app:1"),
            ("lo".into(), "dev/app:1".into())
        );
    }

    #[test]
    fn a_bare_reference_keeps_every_character_it_has() {
        assert_eq!(
            collapse_reference("comicarr:latest"),
            (String::new(), "comicarr:latest".into())
        );
        assert_eq!(
            collapse_reference("team/app:1"),
            (String::new(), "team/app:1".into()),
            "a namespace is not a registry host"
        );
    }

    #[test]
    fn only_docker_hub_loses_its_library_namespace() {
        assert_eq!(
            collapse_reference("ghcr.io/library/thing:1").1,
            "library/thing:1"
        );
    }

    #[test]
    fn eliding_keeps_both_ends_of_a_reference() {
        assert_eq!(elide("short", 10), "short");
        assert_eq!(elide("blakeblackshear/frigate", 12), "blakeb…igate");
        assert_eq!(elide("abcdef", 6), "abcdef");
        assert_eq!(elide("abcdef", 1), "…");
        assert_eq!(elide("abcdef", 0), "");
    }
}
