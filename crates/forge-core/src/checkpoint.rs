//! Durable resume points (AD-8).
//!
//! A thermal pause or a process death resumes at the last completed segment.
//! The checkpoint carries the hash of the plan it was produced under, so a
//! resume against an edited pipeline is refused instead of silently splicing
//! output from two different graphs together.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

use crate::scheduler::SegmentId;
use crate::CoreError;

/// Durable resume point.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobCheckpoint {
    pub job_id: String,
    pub last_segment: SegmentId,
    pub plan_hash: String,
}

impl JobCheckpoint {
    /// A checkpoint recording `last_segment` as complete under `plan_hash`.
    pub fn new(job_id: impl Into<String>, last_segment: SegmentId, plan_hash: impl Into<String>) -> Self {
        JobCheckpoint {
            job_id: job_id.into(),
            last_segment,
            plan_hash: plan_hash.into(),
        }
    }

    /// Whether this checkpoint belongs to the plan identified by `plan_hash`.
    /// A `false` here is what stops a resume producing mixed output.
    pub fn matches_plan(&self, plan_hash: &str) -> bool {
        self.plan_hash == plan_hash
    }
}

/// The SHA-256 of a canonical description of a plan, lower-case hex.
pub fn plan_hash(canonical: &str) -> String {
    let digest = Sha256::digest(canonical.as_bytes());
    let mut hex = String::with_capacity(64);
    for byte in digest {
        use std::fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// Checkpoints on disk, one file per job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckpointStore {
    pub root: PathBuf,
}

impl CheckpointStore {
    /// A store rooted at `root`. The directory is created on first record.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        CheckpointStore { root: root.into() }
    }

    fn path_for(&self, job_id: &str) -> PathBuf {
        // Job ids come from the scheduler, but a path is a boundary: keep the
        // file name to characters that cannot escape the root.
        let safe: String = job_id
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        self.root.join(format!("{safe}.json"))
    }

    /// Persist a checkpoint, replacing any earlier one for the same job.
    ///
    /// Written to a staging file and renamed, so a kill during the write
    /// leaves the previous checkpoint intact rather than a truncated one.
    pub fn record(&self, checkpoint: &JobCheckpoint) -> Result<(), CoreError> {
        std::fs::create_dir_all(&self.root)?;
        let path = self.path_for(&checkpoint.job_id);
        let staging = path.with_extension("partial");
        std::fs::write(&staging, serde_json::to_vec_pretty(checkpoint)?)?;
        std::fs::rename(&staging, &path)?;
        Ok(())
    }

    /// Restore a killed job at its last completed segment.
    ///
    /// `None` means there is nothing to resume — an unknown job, or one that
    /// never completed a segment.
    pub fn resume(&self, job_id: &str) -> Result<Option<JobCheckpoint>, CoreError> {
        let path = self.path_for(job_id);
        if !path.exists() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path)?;
        Ok(Some(serde_json::from_str(&text)?))
    }

    /// Forget a job's checkpoint once it has completed.
    pub fn clear(&self, job_id: &str) -> Result<(), CoreError> {
        let path = self.path_for(job_id);
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdir::TestDir;

    #[test]
    fn resume_returns_none_for_an_unknown_job() {
        let dir = TestDir::new("checkpoint-unknown");
        let store = CheckpointStore::new(dir.path());
        assert_eq!(store.resume("no-such-job").expect("resume"), None);
    }

    #[test]
    fn a_recorded_checkpoint_resumes_at_its_last_segment() {
        let dir = TestDir::new("checkpoint-roundtrip");
        let store = CheckpointStore::new(dir.path());
        let checkpoint = JobCheckpoint::new("job-1", SegmentId(4), "abc123");

        store.record(&checkpoint).expect("record");
        assert_eq!(store.resume("job-1").expect("resume"), Some(checkpoint));
    }

    #[test]
    fn recording_again_replaces_the_earlier_checkpoint() {
        let dir = TestDir::new("checkpoint-replace");
        let store = CheckpointStore::new(dir.path());
        store
            .record(&JobCheckpoint::new("job-1", SegmentId(1), "abc123"))
            .expect("first record");
        store
            .record(&JobCheckpoint::new("job-1", SegmentId(2), "abc123"))
            .expect("second record");

        let resumed = store.resume("job-1").expect("resume").expect("some");
        assert_eq!(resumed.last_segment, SegmentId(2));
    }

    #[test]
    fn a_checkpoint_from_a_different_plan_is_not_matched() {
        let checkpoint = JobCheckpoint::new("job-1", SegmentId(3), plan_hash("graph-a"));
        assert!(checkpoint.matches_plan(&plan_hash("graph-a")));
        assert!(!checkpoint.matches_plan(&plan_hash("graph-b")));
    }

    #[test]
    fn clearing_a_job_removes_its_resume_point() {
        let dir = TestDir::new("checkpoint-clear");
        let store = CheckpointStore::new(dir.path());
        store
            .record(&JobCheckpoint::new("job-1", SegmentId(0), "abc123"))
            .expect("record");
        store.clear("job-1").expect("clear");
        assert_eq!(store.resume("job-1").expect("resume"), None);
    }
}
