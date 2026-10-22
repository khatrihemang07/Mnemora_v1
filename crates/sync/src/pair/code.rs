//! The pairing code: 8 digits typed on one device from the other's screen. It is only a short
//! secret for SPAKE2, so it lives 2 minutes and dies after 3 wrong tries.

use std::time::Duration;

use ring::rand::{SecureRandom, SystemRandom};

/// Digits in a code.
pub const DIGITS: usize = 8;
/// How long a code, and the Allow question after it, may take.
pub const LIFE: Duration = Duration::from_secs(120);
/// Wrong tries after which a code is dead.
pub const MAX_TRIES: u8 = 3;

const SPACE: u64 = 100_000_000;

/// A fresh random code, or `None` if the system has no randomness (never expected).
pub fn generate() -> Option<String> {
    let rng = SystemRandom::new();
    // Drop the top sliver of the range so every code is equally likely.
    let limit = u64::MAX - (u64::MAX % SPACE);
    loop {
        let mut bytes = [0u8; 8];
        rng.fill(&mut bytes).ok()?;
        let n = u64::from_be_bytes(bytes);
        if n < limit {
            return Some(format!("{:0width$}", n % SPACE, width = DIGITS));
        }
    }
}

/// `"48210937"` as `"4821 0937"`.
pub fn display(code: &str) -> String {
    let (a, b) = code.split_at(code.len().min(DIGITS / 2));
    format!("{a} {b}")
}

/// What a person typed or pasted, as 8 digits; spaces and dashes are ignored.
pub fn normalise(input: &str) -> Option<String> {
    let digits: String = input
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .collect();
    (digits.len() == DIGITS && digits.bytes().all(|b| b.is_ascii_digit())).then_some(digits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_code_is_eight_digits_and_they_differ() {
        let a = generate().unwrap();
        assert_eq!(a.len(), DIGITS);
        assert!(a.bytes().all(|b| b.is_ascii_digit()), "{a}");
        let many: std::collections::HashSet<String> = (0..20).filter_map(|_| generate()).collect();
        assert!(many.len() > 15, "codes repeat too much: {many:?}");
    }

    #[test]
    fn a_code_shows_in_two_groups() {
        assert_eq!(display("48210937"), "4821 0937");
        assert_eq!(display("00000042"), "0000 0042");
    }

    #[test]
    fn typed_codes_are_normalised() {
        for ok in ["48210937", "4821 0937", " 4821-0937 ", "4 8 2 1 0 9 3 7"] {
            assert_eq!(normalise(ok).as_deref(), Some("48210937"), "{ok:?}");
        }
        for bad in ["", "4821093", "482109377", "4821 093a", "٤٨٢١٠٩٣٧"] {
            assert_eq!(normalise(bad), None, "{bad:?}");
        }
    }

    /// The reading rule the browser mock also runs (`mock-sync.test.ts`).
    #[test]
    fn typed_codes_match_the_shared_fixture() {
        const RULES: &str = include_str!("../../../../src/lib/ipc/fixtures/sync-rules.json");
        let rules: serde_json::Value = serde_json::from_str(RULES).unwrap();
        for case in rules["code_cases"].as_array().unwrap() {
            assert_eq!(
                normalise(case["input"].as_str().unwrap()).as_deref(),
                case["code"].as_str(),
                "{}",
                case["name"]
            );
        }
    }
}
