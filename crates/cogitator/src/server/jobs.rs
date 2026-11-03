//! The dbt job registry (05 §9): every spawned invocation holds a
//! token the client can cancel. The child runs with kill_on_drop, so
//! aborting the task actually kills the process — LSP progress-cancel
//! is deliberately not used (ProgressValue stays Begin/End).

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use cogitator_error::guidance::Guidance;
use serde::Serialize;
use tokio::task::AbortHandle;

/// The dbt.run/test/compile/parse/sourceFreshness/runSelection answer.
#[derive(Debug, Serialize)]
pub struct JobStarted {
    pub token: u64,
}

/// The dbt.cancel confirmation.
#[derive(Debug, Serialize)]
pub struct Cancelled {
    pub token: u64,
    pub label: String,
}

struct Running {
    abort: AbortHandle,
    label: String,
}

#[derive(Default)]
pub struct DbtJobs {
    next: AtomicU64,
    inner: Mutex<HashMap<u64, Running>>,
}

impl DbtJobs {
    /// Reserve the token the spawned task will report under.
    pub fn next_token(&self) -> u64 {
        self.next.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Register a spawned job; finished entries self-clean here and in
    /// cancel/list (the fusion_jobs idiom).
    pub fn register(&self, token: u64, label: String, abort: AbortHandle) {
        let mut map = self.inner.lock().expect("jobs lock");
        map.retain(|_, job| !job.abort.is_finished());
        map.insert(token, Running { abort, label });
    }

    /// Abort the job (killing its child via kill_on_drop).
    pub fn cancel(&self, token: u64) -> Result<Cancelled, Guidance> {
        let mut map = self.inner.lock().expect("jobs lock");
        map.retain(|_, job| !job.abort.is_finished());
        match map.remove(&token) {
            Some(job) => {
                job.abort.abort();
                Ok(Cancelled {
                    token,
                    label: job.label,
                })
            }
            None => {
                let running: Vec<String> = map
                    .iter()
                    .map(|(t, job)| format!("{t} ({})", job.label))
                    .collect();
                Err(
                    Guidance::new(format!("no running dbt job has token {token}."))
                        .because(if running.is_empty() {
                            "nothing is running right now.".to_string()
                        } else {
                            format!("running: {}.", running.join(", "))
                        })
                        .fix("tokens come from the JobStarted answer of dbt.run/test/… commands."),
                )
            }
        }
    }

    /// Running jobs, oldest token first (dbt.doctor reports these).
    pub fn list(&self) -> Vec<(u64, String)> {
        let mut map = self.inner.lock().expect("jobs lock");
        map.retain(|_, job| !job.abort.is_finished());
        let mut jobs: Vec<(u64, String)> =
            map.iter().map(|(t, job)| (*t, job.label.clone())).collect();
        jobs.sort_by_key(|(t, _)| *t);
        jobs
    }
}

/// dbt.runSelection's selector, validated before it becomes argv: dbt
/// selector syntax only, never something argument-shaped.
pub fn validate_selector(selector: &str) -> Result<(), Guidance> {
    let reject = |why: String| {
        Err(
            Guidance::new(format!("'{selector}' is not a usable dbt selector."))
                .because(why)
                .suggest(vec!["tag:nightly".into(), "state:modified+".into()])
                .fix("pass a dbt node selector, as in `dbt run --select <selector>`."),
        )
    };
    if selector.trim().is_empty() {
        return reject("it is empty.".to_string());
    }
    if selector.starts_with('-') {
        return reject("it starts with '-', which would read as a flag.".to_string());
    }
    if let Some(bad) = selector
        .chars()
        .find(|c| !c.is_ascii_alphanumeric() && !"_.:+*@,/ -".contains(*c))
    {
        return reject(format!(
            "the character '{bad}' is not part of selector syntax."
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_validation_rejects_flaggy_input() {
        assert!(validate_selector("tag:nightly").is_ok());
        assert!(validate_selector("state:modified+").is_ok());
        assert!(validate_selector("marts.finance,tag:daily").is_ok());
        assert!(validate_selector("").is_err());
        assert!(validate_selector("-rf").is_err());
        assert!(validate_selector("x; rm").is_err());
        let err = validate_selector("--vars '{}'").unwrap_err().to_message();
        assert!(err.contains("flag"), "{err}");
    }

    #[tokio::test]
    async fn cancel_removes_and_aborts() {
        let jobs = DbtJobs::default();
        let token = jobs.next_token();
        let handle = tokio::spawn(async {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        });
        jobs.register(token, "dbt run: customers".into(), handle.abort_handle());
        assert_eq!(jobs.list().len(), 1);
        let cancelled = jobs.cancel(token).expect("cancels");
        assert_eq!(cancelled.label, "dbt run: customers");
        assert!(handle.await.unwrap_err().is_cancelled());
        let missing = jobs.cancel(token).unwrap_err().to_message();
        assert!(missing.contains("no running dbt job"), "{missing}");
    }
}
