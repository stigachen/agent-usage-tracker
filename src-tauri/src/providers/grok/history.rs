use super::{auth, UsageSnapshot};
use std::path::{Path, PathBuf};

struct LastSuccess {
    path: PathBuf,
    identity: auth::Identity,
    snapshot: UsageSnapshot,
}

/// One account in memory only. No tokens, disk persistence, or cross-account cache lookup.
#[derive(Default)]
pub(super) struct History(Option<LastSuccess>);

impl History {
    pub fn clear(&mut self) {
        self.0 = None;
    }

    pub fn retain_account(&mut self, path: &Path, auth: &auth::Auth) {
        if self.0.as_ref().is_some_and(|last| last.path != path || last.identity != auth.identity) {
            self.clear();
        }
    }

    pub fn finish(&mut self, path: &Path, auth: &auth::Auth, mut snapshot: UsageSnapshot) -> UsageSnapshot {
        self.retain_account(path, auth);
        snapshot.fetched_at = chrono::Utc::now().to_rfc3339();
        if snapshot.needs_auth || snapshot.error.is_some() || snapshot.credential_issue.is_some() || snapshot.fetch_issue.is_some() {
            if let Some(last) = &self.0 {
                snapshot.windows = last.snapshot.windows.clone();
                snapshot.plan = last.snapshot.plan.clone();
                snapshot.period_ends_at = last.snapshot.period_ends_at.clone();
                snapshot.last_success_at = last.snapshot.last_success_at.clone();
                snapshot.stale = true;
            }
        } else if !snapshot.windows.is_empty() {
            snapshot.last_success_at = Some(snapshot.fetched_at.clone());
            self.0 = Some(LastSuccess {
                path: path.to_owned(),
                identity: auth.identity.clone(),
                snapshot: snapshot.clone(),
            });
        } else {
            // A successful response that no longer reports quotas supersedes the old data.
            self.clear();
        }
        snapshot
    }
}
