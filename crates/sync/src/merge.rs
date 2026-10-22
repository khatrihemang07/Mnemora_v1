//! The merge rule for one unit of fields (a single field, or an atomic group). Kept apart so
//! each class has one place to change: three-way text merge (ticket "Conflicts") slots into
//! the `Text` arm.

use mnemora_core::hlc::Stamp;
use mnemora_core::syncable::FieldClass;

/// What to do with a received unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Write the received values.
    Take,
    /// Keep what this device has.
    Keep,
}

/// Decides one unit: `stored` is the newest clock this device knows for it (none when it has
/// never been written), `incoming` the received record's clock. The same record twice, or an
/// older one, keeps what is there; ties on the clock are settled by device id, so every device
/// makes the same choice.
pub fn decide(class: FieldClass, stored: Option<&Stamp>, incoming: &Stamp) -> Verdict {
    match class {
        // Text is newest-wins until the three-way merge arrives; it will diverge here, where
        // the base and both texts are known.
        FieldClass::Text | FieldClass::Value | FieldClass::Order => newest(stored, incoming),
        FieldClass::Derived => Verdict::Keep,
    }
}

fn newest(stored: Option<&Stamp>, incoming: &Stamp) -> Verdict {
    match stored {
        Some(stored) if incoming <= stored => Verdict::Keep,
        _ => Verdict::Take,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stamp(ms: i64, counter: i64, device: &str) -> Stamp {
        Stamp {
            ms,
            counter,
            device: device.into(),
        }
    }

    #[test]
    fn newest_clock_wins_and_ties_go_by_device() {
        let old = stamp(5, 0, "a");
        assert_eq!(
            decide(FieldClass::Value, Some(&old), &stamp(6, 0, "a")),
            Verdict::Take
        );
        assert_eq!(
            decide(FieldClass::Value, Some(&old), &stamp(5, 1, "a")),
            Verdict::Take
        );
        assert_eq!(
            decide(FieldClass::Value, Some(&old), &stamp(5, 0, "b")),
            Verdict::Take
        );
        assert_eq!(
            decide(FieldClass::Value, Some(&stamp(5, 0, "b")), &old),
            Verdict::Keep
        );
        assert_eq!(
            decide(FieldClass::Order, Some(&old), &old),
            Verdict::Keep,
            "the same record twice"
        );
        assert_eq!(
            decide(FieldClass::Text, None, &old),
            Verdict::Take,
            "nothing there yet"
        );
    }

    #[test]
    fn derived_fields_never_take() {
        assert_eq!(
            decide(FieldClass::Derived, None, &stamp(1, 0, "a")),
            Verdict::Keep
        );
    }
}
