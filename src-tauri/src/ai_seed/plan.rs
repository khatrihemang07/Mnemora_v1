//! What the seed does with a Sandbox database in a given state: write, reset first, or keep it.

use super::database::DbState;

/// What `seed_at` does with a database in a given state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Plan {
    /// Write the fixture into the database as it is.
    Write,
    /// Delete the Sandbox database files, then write the fixture.
    Reset,
    /// Write nothing: the fixture is already there.
    Keep,
}

/// What to do with a database in `state`. Refuses when it holds rows that are not the fixture's
/// unless `reset` is given.
pub(super) fn plan(state: DbState, reset: bool) -> Result<Plan, String> {
    match (state, reset) {
        (DbState::Missing | DbState::Empty, _) => Ok(Plan::Write),
        (DbState::Seeded, false) => Ok(Plan::Keep),
        (DbState::Seeded | DbState::Other, true) => Ok(Plan::Reset),
        (DbState::Other, false) => Err(
            "the Sandbox database holds rows that are not the fixture, so nothing was written. \
             Pass --reset to delete the Sandbox database and seed it again."
                .to_owned(),
        ),
    }
}
