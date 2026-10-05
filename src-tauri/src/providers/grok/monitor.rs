use super::auth;
use chrono::{DateTime, Utc};
use std::path::PathBuf;

// A fingerprint contains no bearer. Full identities, not fingerprints, isolate usage history.
#[derive(Clone, PartialEq, Eq)]
pub(super) struct Stamp {
    path: Option<PathBuf>,
    state: Result<(u64, bool), auth::AuthError>,
}

impl Stamp {
    pub fn read(path: Result<PathBuf, auth::AuthError>, now: DateTime<Utc>) -> Self {
        match path {
            Ok(path) => {
                let auth = auth::read(&path);
                Self::from_read(Some(path), auth.as_ref().map_err(|error| *error), now)
            }
            Err(error) => Self { path: None, state: Err(error) },
        }
    }

    pub fn from_read(path: Option<PathBuf>, auth: Result<&auth::Auth, auth::AuthError>, now: DateTime<Utc>) -> Self {
        Self { path, state: auth.map(|auth| (auth.fingerprint(), auth.expired(now))) }
    }
}

/// Two equal observations debounce partial writes and replacements without OS-specific watchers.
/// Stable observations never repeatedly schedule requests; creation/deletion and expiry are observable.
#[derive(Default)]
pub(super) struct Monitor {
    applied: Option<Stamp>,
    observed: Option<Stamp>,
    retry_pending: bool,
}

impl Monitor {
    /// Use the credentials actually read by a request as its baseline. In particular, a transient
    /// read failure must be observable even if the original file returns before the next poll.
    pub fn attempted(&mut self, stamp: Stamp) {
        self.applied = Some(stamp.clone());
        self.observed = Some(stamp);
        self.retry_pending = false;
    }

    /// A discarded result needs another query even if the original credentials return.
    /// Restart the debounce so the retry waits for two matching observations.
    pub fn retry_after_change(&mut self) {
        self.retry_pending = true;
        self.observed = None;
    }

    pub fn observe(&mut self, stamp: Stamp) -> bool {
        if self.applied.is_none() {
            self.attempted(stamp);
            return false;
        }
        if self.observed.as_ref() != Some(&stamp) {
            self.observed = Some(stamp);
            return false;
        }
        if self.applied.as_ref() == Some(&stamp) && !self.retry_pending {
            return false;
        }
        self.applied = Some(stamp);
        self.retry_pending = false;
        true
    }
}
