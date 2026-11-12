//! How long ago something happened, in its largest whole unit.

use std::time::SystemTime;

use gpui::SharedString;

/// How long before `as_of` something happened, as the palette and the notice list print it:
/// `4m`, `2h`, `3d`, `5w`.
pub(crate) fn age(as_of: Option<SystemTime>, time: SystemTime) -> SharedString {
    let Some(seconds) = as_of
        .and_then(|as_of| as_of.duration_since(time).ok())
        .map(|age| age.as_secs())
    else {
        return SharedString::default();
    };
    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;
    const WEEK: u64 = 7 * DAY;
    let text = match seconds {
        s if s < HOUR => format!("{}m", (s / MINUTE).max(1)),
        s if s < DAY => format!("{}h", s / HOUR),
        s if s < WEEK => format!("{}d", s / DAY),
        s if s < 52 * WEEK => format!("{}w", s / WEEK),
        s => format!("{}y", s / (52 * WEEK)),
    };
    text.into()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn ages_use_the_largest_whole_unit() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000_000);
        let ago = |seconds| age(Some(now), now - Duration::from_secs(seconds));
        assert_eq!(ago(30), "1m");
        assert_eq!(ago(26 * 60), "26m");
        assert_eq!(ago(3 * 60 * 60), "3h");
        assert_eq!(ago(3 * 24 * 60 * 60), "3d");
        assert_eq!(ago(5 * 7 * 24 * 60 * 60), "5w");
    }
}
