//! Fractional-index keys: the text that keeps Tasks (and Lists) in their manual order. A key sorts
//! bytewise, and there is always a key strictly between any two others, so a move rewrites one row
//! and never renumbers the rest (ADR 0012). Pure functions, no database.
//!
//! A key is a string of base-62 digits read as a fraction in (0, 1): `V` is one half. Digits are in
//! byte order, so comparing the strings compares the fractions. A canonical key never ends in `0`
//! (trailing zeros add nothing to a fraction but would leave no room between two keys); keys made
//! before this module existed did, and are read as if they did not.

use crate::error::TodoError;

/// Base-62 digits in byte order, so keys sort the same as text and as fractions.
const DIGITS: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
/// The middle digit, which the first key starts from.
const MID: usize = 31;

/// The digit's value, or none for a byte that is not a digit.
fn value(b: u8) -> Option<usize> {
    DIGITS.iter().position(|d| *d == b)
}

/// The key without trailing zeros: how it is compared. An all-zero key is empty (the lowest).
fn canonical(key: &str) -> &str {
    key.trim_end_matches('0')
}

/// Whether the text is a usable key: base-62 digits only.
fn is_key(key: &str) -> bool {
    key.bytes().all(|b| value(b).is_some())
}

/// A key strictly between `lo` and `hi`; `None` means no bound on that side (before the first key,
/// after the last, or, with both none, the very first key). Fails when a bound is not a key or
/// `lo` is not below `hi` once trailing zeros are ignored: callers fall back to a neighbour.
pub fn key_between(lo: Option<&str>, hi: Option<&str>) -> Result<String, TodoError> {
    const BAD: TodoError = TodoError::Validation("Those two places cannot be told apart");
    let (lo, hi) = (lo.map(canonical), hi.map(canonical));
    if lo.is_some_and(|k| !is_key(k)) || hi.is_some_and(|k| !is_key(k)) {
        return Err(BAD);
    }
    if let (Some(lo), Some(hi)) = (lo, hi)
        && lo >= hi
    {
        return Err(BAD);
    }
    // Nothing sorts below the empty key, so a bound of it leaves no room.
    if hi == Some("") {
        return Err(BAD);
    }
    Ok(midpoint(lo.unwrap_or(""), hi))
}

/// The key that goes after every other, given the last. Appends stay short: the first digit that
/// can still grow is stepped up and the rest dropped (`V` then `W`, `X`, … `z`, then `zV`, `zW`,
/// …), so a digit is added only about once in thirty-one appends.
pub fn key_after(last: Option<&str>) -> String {
    let last = last.map_or("", canonical);
    if !is_key(last) {
        return format!("{last}{}", DIGITS[MID] as char);
    }
    let top = DIGITS.len() - 1;
    match last.bytes().position(|b| value(b).is_some_and(|v| v < top)) {
        Some(i) => {
            let step = value(last.as_bytes()[i]).map_or(MID, |v| v + 1);
            format!("{}{}", &last[..i], DIGITS[step] as char)
        }
        // Empty, or all `z`: nothing to step.
        None => format!("{last}{}", DIGITS[MID] as char),
    }
}

/// Rocicorp's midpoint: `lo` is canonical or empty, `hi` canonical or none (the end), `lo < hi`.
fn midpoint(lo: &str, hi: Option<&str>) -> String {
    if let Some(hi) = hi {
        // Skip the digits both share, treating `lo` as padded with zeros.
        let (l, h) = (lo.as_bytes(), hi.as_bytes());
        let mut n = 0;
        while n < h.len() && l.get(n).copied().unwrap_or(b'0') == h[n] {
            n += 1;
        }
        if n > 0 {
            let rest_lo = lo.get(n..).unwrap_or("");
            return format!("{}{}", &hi[..n], midpoint(rest_lo, Some(&hi[n..])));
        }
    }
    let digit_lo = lo.bytes().next().and_then(value).unwrap_or(0);
    let digit_hi = hi
        .and_then(|h| h.bytes().next())
        .and_then(value)
        .unwrap_or(DIGITS.len());
    if digit_hi - digit_lo > 1 {
        return (DIGITS[(digit_lo + digit_hi).div_ceil(2)] as char).to_string();
    }
    // Adjacent first digits: a longer `hi` has room below it at its first digit; otherwise stay on
    // `lo`'s digit and look for room after the rest of `lo`.
    if let Some(hi) = hi
        && hi.len() > 1
    {
        return hi[..1].to_owned();
    }
    let rest = lo.get(1..).unwrap_or("");
    format!("{}{}", DIGITS[digit_lo] as char, midpoint(rest, None))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn between(lo: Option<&str>, hi: Option<&str>) -> String {
        let key = key_between(lo, hi).unwrap();
        assert!(!key.ends_with('0'), "{key} ends in a zero");
        assert!(is_key(&key));
        if let Some(lo) = lo {
            assert!(key.as_str() > canonical(lo), "{key} should follow {lo}");
        }
        if let Some(hi) = hi {
            assert!(key.as_str() < canonical(hi), "{key} should precede {hi}");
        }
        key
    }

    #[test]
    fn the_first_key_is_mid_range() {
        assert_eq!(between(None, None), "V");
    }

    #[test]
    fn a_key_fits_between_any_two_neighbours() {
        let keys = [
            "1", "V", "W", "Vk", "z", "zz", "0V", "00001", "a0b", "Vz", "VzzV",
        ];
        for lo in keys {
            for hi in keys {
                if lo < hi {
                    between(Some(lo), Some(hi));
                }
            }
        }
    }

    #[test]
    fn a_key_fits_before_the_first_and_after_the_last() {
        let mut first = "V".to_owned();
        for _ in 0..60 {
            let before = between(None, Some(&first));
            assert!(before < first);
            first = before;
        }
        let mut last = "V".to_owned();
        for _ in 0..200 {
            let after = key_after(Some(&last));
            assert!(after > last, "{after} should follow {last}");
            assert!(!after.ends_with('0'));
            last = after;
        }
        assert!(between(Some("zzz"), None).as_str() > "zzz");
    }

    #[test]
    fn appends_stay_short() {
        let mut last = None::<String>;
        for _ in 0..500 {
            last = Some(key_after(last.as_deref()));
        }
        assert!(last.unwrap().len() <= 18);
    }

    #[test]
    fn many_inserts_in_the_same_gap_stay_ordered_without_renumbering() {
        // Always insert directly after the lower neighbour: the worst case for key growth.
        let (lo, hi) = ("V".to_owned(), "W".to_owned());
        let mut mid = lo.clone();
        let mut made = Vec::new();
        for _ in 0..1000 {
            mid = between(Some(&mid), Some(&hi));
            made.push(mid.clone());
        }
        assert!(made.windows(2).all(|w| w[0] < w[1]));
        assert!(
            made.iter()
                .all(|k| k.as_str() > lo.as_str() && k.as_str() < hi.as_str())
        );
        // And directly before the upper neighbour.
        let mut mid = hi.clone();
        let mut made = Vec::new();
        for _ in 0..1000 {
            mid = between(Some(&lo), Some(&mid));
            made.push(mid.clone());
        }
        assert!(made.windows(2).all(|w| w[0] > w[1]));
        // Keys grow by about a digit per six inserts: far from unbounded.
        assert!(mid.len() < 400, "{} digits", mid.len());
    }

    #[test]
    fn random_moves_keep_a_list_sorted() {
        // A deterministic shuffle: move random items to random places, only ever writing the
        // moved item's key.
        let mut seed = 0x2545_F491_4F6C_DD1D_u64;
        let mut next = move |n: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            usize::try_from(seed % n as u64).unwrap()
        };
        let mut keys: Vec<String> = Vec::new();
        for _ in 0..40 {
            keys.push(key_after(keys.last().map(String::as_str)));
        }
        for _ in 0..3000 {
            let from = next(keys.len());
            keys.remove(from);
            let at = next(keys.len() + 1);
            let lo = at.checked_sub(1).map(|i| keys[i].as_str());
            let hi = keys.get(at).map(String::as_str);
            let key = between(lo, hi);
            keys.insert(at, key);
        }
        assert!(keys.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn old_keys_with_trailing_zeros_keep_working() {
        // The keys of the first Todo slices: eight digits, counting up from V0000000.
        assert_eq!(key_after(Some("V0000000")), "W");
        let key = between(Some("V0000000"), Some("V0000001"));
        assert!(key.as_str() > "V0000000" && key.as_str() < "V0000001");
        // `V` and `V0000000` are the same place: nothing fits between them.
        assert!(key_between(Some("V"), Some("V0000000")).is_err());
        assert!(key_between(Some("W"), Some("V")).is_err());
        assert!(key_between(Some("V"), Some("V")).is_err());
        assert!(key_between(Some("a b"), None).is_err());
        assert!(key_between(None, Some("000")).is_err());
    }

    #[test]
    fn a_key_in_some_other_shape_still_gets_a_later_one() {
        assert!(key_after(Some("a b")).as_str() > "a b");
    }
}
