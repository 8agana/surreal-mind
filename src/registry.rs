//! Generic job registry for tracking running async jobs and enabling cancellation
//!
//! This registry maintains a global mapping of job_id -> AbortHandle, allowing
//! external tools (call_cancel, etc.) to abort running jobs immediately without
//! waiting for polling intervals.
//!
//! Designed to be generic across delegate_gemini, call_cc, and future workers.

use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::sync::Arc;

/// Wraps a tokio task AbortHandle for storage in the registry
pub struct AbortHandleInfo {
    pub handle: tokio::task::JoinHandle<()>,
}

/// Global job registry: job_id -> AbortHandleInfo
static JOB_REGISTRY: Lazy<Arc<DashMap<String, AbortHandleInfo>>> =
    Lazy::new(|| Arc::new(DashMap::new()));

/// Register a running job in the registry
///
/// # Arguments
/// * `job_id` - Unique identifier for the job
/// * `handle` - JoinHandle of the spawned task
pub fn register_job(job_id: String, handle: tokio::task::JoinHandle<()>) {
    JOB_REGISTRY.insert(job_id, AbortHandleInfo { handle });
}

/// Unregister a job from the registry
///
/// # Arguments
/// * `job_id` - Unique identifier for the job
pub fn unregister_job(job_id: &str) {
    JOB_REGISTRY.remove(job_id);
}

/// Abort a running job by job_id, if it exists in the registry
///
/// Returns true if the job was found and aborted, false if not found.
/// This is safe to call multiple times (idempotent).
///
/// # Arguments
/// * `job_id` - Unique identifier for the job
pub fn abort_job(job_id: &str) -> bool {
    if let Some((_, info)) = JOB_REGISTRY.remove(job_id) {
        info.handle.abort();
        true
    } else {
        false
    }
}

/// Get the number of jobs currently registered
///
/// Useful for monitoring and testing.
pub fn registry_size() -> usize {
    JOB_REGISTRY.len()
}

/// Clear all jobs from the registry
///
/// WARNING: This aborts all running jobs. Use only in testing or controlled shutdown.
#[cfg(test)]
pub fn clear_registry() {
    for entry in JOB_REGISTRY.iter() {
        entry.value().handle.abort();
    }
    JOB_REGISTRY.clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_and_abort() {
        let handle =
            tokio::spawn(async { tokio::time::sleep(std::time::Duration::from_secs(10)).await });
        let job_id = format!("test-job-1-{}", uuid::Uuid::new_v4());

        register_job(job_id.clone(), handle);
        let was_registered = registry_size() > 0;
        assert!(was_registered);

        let aborted = abort_job(&job_id);
        assert!(aborted);
    }

    #[tokio::test]
    async fn test_abort_nonexistent_job() {
        let aborted = abort_job("nonexistent-job-that-does-not-exist-12345");
        assert!(!aborted);
    }

    #[tokio::test]
    async fn test_unregister() {
        let handle = tokio::spawn(async {});
        let job_id = format!("test-job-2-{}", uuid::Uuid::new_v4());
        let initial_size = registry_size();

        register_job(job_id.clone(), handle);
        assert!(registry_size() > initial_size);

        unregister_job(&job_id);
        assert_eq!(registry_size(), initial_size);
    }

    #[tokio::test]
    async fn test_idempotent_abort() {
        let handle = tokio::spawn(async {});
        let job_id = format!("test-job-3-{}", uuid::Uuid::new_v4());

        register_job(job_id.clone(), handle);

        let first_abort = abort_job(&job_id);
        let second_abort = abort_job(&job_id);

        assert!(first_abort);
        assert!(!second_abort);
    }
}
