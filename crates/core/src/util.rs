//! Tiny helpers shared across the workspace.

use std::time::{SystemTime, UNIX_EPOCH};

/// A new record id. `UUIDv7` is time-ordered, which keeps B-tree inserts local and sync merges sane.
pub fn new_id() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// Milliseconds since the Unix epoch (UTC).
pub fn now_ms() -> i64 {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(since_epoch.as_millis()).unwrap_or(i64::MAX)
}

/// Formats epoch milliseconds as sortable `YYYYMMDDTHHMMSSmmm` (UTC), safe in file names.
pub fn compact_utc(ms: i64) -> String {
    let days = ms.div_euclid(86_400_000);
    let ms_of_day = ms.rem_euclid(86_400_000);
    let (year, month, day) = civil_from_days(days);
    let (hour, minute, second, milli) = (
        ms_of_day / 3_600_000,
        ms_of_day / 60_000 % 60,
        ms_of_day / 1000 % 60,
        ms_of_day % 1000,
    );
    format!("{year:04}{month:02}{day:02}T{hour:02}{minute:02}{second:02}{milli:03}")
}

/// Days since 1970-01-01 to (year, month, day); Howard Hinnant's `civil_from_days`.
/// Hand-rolled to avoid a date-time dependency for one file-name format.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_id_is_uuid_v7_and_unique() {
        let a = new_id();
        let b = new_id();
        assert_ne!(a, b);
        assert_eq!(a.len(), 36);
        // Version nibble sits at the start of the third group.
        assert_eq!(a.as_bytes()[14], b'7');
    }

    #[test]
    fn new_ids_sort_by_creation_time() {
        let a = new_id();
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = new_id();
        assert!(a < b);
    }

    #[test]
    fn now_ms_is_after_2024() {
        assert!(now_ms() > 1_704_067_200_000);
    }

    #[test]
    fn compact_utc_formats_known_instants() {
        assert_eq!(compact_utc(0), "19700101T000000000");
        // 2024-02-29T12:34:56.789Z, a leap day.
        assert_eq!(compact_utc(1_709_210_096_789), "20240229T123456789");
    }
}
