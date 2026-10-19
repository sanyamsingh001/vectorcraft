//! Pure comparison of the installed version with GitHub's latest published stable release.
//! HTTP and dialogs belong to the frontend; this module neither reads preferences nor does I/O.

use std::cmp::Ordering;

use semver::Version;
use serde::{Deserialize, Serialize};

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const LATEST_RELEASE_API: &str = "https://api.github.com/repos/storytold/vectorcraft/releases/latest";
pub const RELEASES_URL: &str = "https://github.com/storytold/vectorcraft/releases/latest";
const RELEASE_TAG_URL: &str = "https://github.com/storytold/vectorcraft/releases/tag/";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VersionStatus {
    UpdateAvailable,
    UpToDate,
    Ahead,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseCheck {
    pub current_version: String,
    pub latest_version: String,
    pub release_url: String,
    pub status: VersionStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReleaseError {
    NoStableRelease,
    Http(u16),
    InvalidResponse,
    NotStable,
    InvalidVersion,
    InvalidCurrentVersion,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
}

/// Compare semantic-version precedence, ignoring build metadata. Links always identify this
/// repository's returned stable tag; no server-supplied URL or release asset is opened.
pub fn compare_latest(current: &str, http_status: u16, body: &[u8]) -> Result<ReleaseCheck, ReleaseError> {
    if http_status == 404 {
        return Err(ReleaseError::NoStableRelease);
    }
    if http_status != 200 {
        return Err(ReleaseError::Http(http_status));
    }
    let release: Release = serde_json::from_slice(body).map_err(|_| ReleaseError::InvalidResponse)?;
    let latest = Version::parse(release.tag_name.strip_prefix('v').unwrap_or(&release.tag_name)).map_err(|_| ReleaseError::InvalidVersion)?;
    if release.draft || release.prerelease || !latest.pre.is_empty() {
        return Err(ReleaseError::NotStable);
    }
    let current = Version::parse(current).map_err(|_| ReleaseError::InvalidCurrentVersion)?;
    let status = match latest.cmp_precedence(&current) {
        Ordering::Greater => VersionStatus::UpdateAvailable,
        Ordering::Equal => VersionStatus::UpToDate,
        Ordering::Less => VersionStatus::Ahead,
    };
    Ok(ReleaseCheck {
        current_version: current.to_string(),
        latest_version: latest.to_string(),
        release_url: format!("{RELEASE_TAG_URL}{}", release.tag_name),
        status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({"tag_name": tag, "draft": false, "prerelease": false})).unwrap()
    }

    #[test]
    fn stable_versions_compare_numerically_and_link_to_the_returned_tag() {
        let current = compare_latest("0.7.0", 200, &release("v0.7.0")).unwrap();
        assert_eq!(current.status, VersionStatus::UpToDate);
        assert_eq!(current.latest_version, "0.7.0");
        assert_eq!(current.release_url, "https://github.com/storytold/vectorcraft/releases/tag/v0.7.0");
        let newer = compare_latest("1.9.0", 200, &release("v1.10.0")).unwrap();
        assert_eq!(newer.status, VersionStatus::UpdateAvailable);
        assert_eq!(compare_latest("1.10.0", 200, &release("1.9.0")).unwrap().status, VersionStatus::Ahead);
    }

    #[test]
    fn a_stable_release_updates_its_prerelease_but_build_metadata_is_not_an_update() {
        assert_eq!(compare_latest("0.7.0-rc.1", 200, &release("v0.7.0")).unwrap().status, VersionStatus::UpdateAvailable);
        assert_eq!(compare_latest("0.7.0+local", 200, &release("v0.7.0")).unwrap().status, VersionStatus::UpToDate);
        assert_eq!(compare_latest("0.7.0+local", 200, &release("v0.7.0+release")).unwrap().status, VersionStatus::UpToDate);
        assert_eq!(compare_latest("0.9.0-rc.1", 200, &release("v0.8.0")).unwrap().status, VersionStatus::Ahead);
    }

    #[test]
    fn unavailable_incomplete_and_nonstable_release_responses_are_errors() {
        assert_eq!(compare_latest("0.7.0", 404, b"{}"), Err(ReleaseError::NoStableRelease));
        for status in [403, 429, 503] {
            assert_eq!(compare_latest("0.7.0", status, b"{}"), Err(ReleaseError::Http(status)));
        }
        assert_eq!(compare_latest("0.7.0", 200, b"{}"), Err(ReleaseError::InvalidResponse));
        assert_eq!(compare_latest("0.7.0", 200, &release("release")), Err(ReleaseError::InvalidVersion));
        assert_eq!(compare_latest("0.7.0", 200, &release("vv0.7.0")), Err(ReleaseError::InvalidVersion));
        assert_eq!(compare_latest("unknown", 200, &release("v0.7.0")), Err(ReleaseError::InvalidCurrentVersion));
        for body in [
            serde_json::json!({"tag_name":"v0.8.0", "draft": true, "prerelease": false}),
            serde_json::json!({"tag_name":"v0.8.0", "draft": false, "prerelease": true}),
            serde_json::json!({"tag_name":"v0.8.0-rc.1", "draft": false, "prerelease": false}),
        ] {
            assert_eq!(compare_latest("0.7.0", 200, &serde_json::to_vec(&body).unwrap()), Err(ReleaseError::NotStable));
        }
    }
}
