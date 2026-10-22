//! When two devices dial each other at the same moment they end up with two connections. Both
//! must keep the same one, so the rule uses only what both can see: the connection dialled by the
//! device with the lower id wins.

/// Whether a newly established connection should replace the one already held with the same
/// device. `existing_by_me` and `new_by_me` say whether this device dialled that connection.
///
/// A connection dialled by the same side as the held one replaces it: the old one is likely
/// half-dead (the other end restarted or the network changed), and the new one proves itself.
pub fn replaces(me: &str, peer: &str, existing_by_me: bool, new_by_me: bool) -> bool {
    if existing_by_me == new_by_me {
        return true;
    }
    // The dialler of the new connection is lower than the dialler of the old one.
    let dialler = |by_me: bool| if by_me { me } else { peer };
    dialler(new_by_me) < dialler(existing_by_me)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_connection_dialled_by_the_lower_id_is_kept_whichever_arrives_first() {
        // "a" < "b": the connection dialled by a wins.
        assert!(
            !replaces("a", "b", true, false),
            "a holds its own, b's loses"
        );
        assert!(
            replaces("a", "b", false, true),
            "b's is held, a's replaces it"
        );
        // The same pair seen from the other device agrees: b holds a's and drops its own.
        assert!(replaces("b", "a", true, false), "b holds its own, a's wins");
        assert!(!replaces("b", "a", false, true), "a's is held, b's loses");
    }

    #[test]
    fn both_devices_keep_the_same_connection() {
        // Connection X dialled by a, Y dialled by b. Whatever order each device sees them in, both
        // end holding X.
        let a_holds_x_then_y = !replaces("a", "b", true, false);
        let a_holds_y_then_x = replaces("a", "b", false, true);
        let b_holds_x_then_y = !replaces("b", "a", false, true);
        let b_holds_y_then_x = replaces("b", "a", true, false);
        assert!(a_holds_x_then_y && a_holds_y_then_x && b_holds_x_then_y && b_holds_y_then_x);
    }

    #[test]
    fn a_second_connection_from_the_same_dialler_replaces_the_first() {
        assert!(replaces("a", "b", true, true));
        assert!(replaces("a", "b", false, false));
    }
}
