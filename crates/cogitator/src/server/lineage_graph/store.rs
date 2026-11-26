//! One project's cached model lineages, and the queue that fills them.
//!
//! An entry is valid while its model's [`ModelKey`] holds; a changed model
//! is computed again, nothing else. Work is queued (a request's models go
//! first) and run by a few worker threads of the store's own, never on
//! the dispatcher or a request: a request waits for what it needs up to a
//! deadline and answers with the rest marked stale. When the queue runs
//! dry the last worker writes the store to disk.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Instant;

use cogitator_analysis::lineage::ModelLineage;

use super::persist;
use super::stamps::{Keyed, ModelKey};

/// Computes one model's lineage: (lineage, read the compiled file).
pub type Compute = Arc<dyn Fn(&str) -> (Option<ModelLineage>, bool) + Send + Sync>;

#[derive(Clone)]
pub struct Entry {
    pub base: u64,
    /// `Some(compiled key)` when the answer came from the compiled route.
    pub compiled: Option<u64>,
    pub lineage: Arc<Option<ModelLineage>>,
}

impl Entry {
    pub fn holds(&self, key: &ModelKey) -> bool {
        self.base == key.base && self.compiled.is_none_or(|c| c == key.compiled)
    }
}

struct Job {
    keyed: Keyed,
    compute: Compute,
}

#[derive(Default)]
struct State {
    entries: HashMap<String, Entry>,
    queue: VecDeque<Job>,
    queued: HashSet<String>,
    running: HashSet<String>,
    workers: usize,
    dirty: bool,
    /// The project's models at the last request: what a save keeps.
    live: Option<HashSet<String>>,
}

/// Counters for dbt.cache / dbt.status.
#[derive(Default)]
pub struct Stats {
    pub hits: AtomicU64,
    pub computed: AtomicU64,
    pub loaded: AtomicU64,
    pub saves: AtomicU64,
}

pub struct Store {
    root: PathBuf,
    state: Mutex<State>,
    changed: Condvar,
    pub stats: Stats,
    max_workers: usize,
}

impl Store {
    /// The store of `root`, filled from disk when a cache file of this
    /// build is there.
    pub fn open(root: PathBuf, max_workers: usize) -> Self {
        let entries = persist::load(&root).unwrap_or_default();
        let stats = Stats::default();
        stats.loaded.store(entries.len() as u64, Ordering::Relaxed);
        Store {
            root,
            state: Mutex::new(State {
                entries,
                ..State::default()
            }),
            changed: Condvar::new(),
            stats,
            max_workers: max_workers.max(1),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The entry of `model` if it holds for `key`.
    pub fn valid(&self, model: &str, key: &ModelKey) -> Option<Arc<Option<ModelLineage>>> {
        let state = self.lock();
        let entry = state.entries.get(model).filter(|e| e.holds(key))?;
        self.stats.hits.fetch_add(1, Ordering::Relaxed);
        Some(entry.lineage.clone())
    }

    /// Queues every model whose entry does not hold; `urgent` ones jump
    /// the queue. Returns the names now queued or running for them.
    pub fn ensure(
        self: &Arc<Self>,
        models: impl IntoIterator<Item = Keyed>,
        compute: &Compute,
        urgent: bool,
    ) -> Vec<String> {
        let mut state = self.lock();
        let mut waiting = Vec::new();
        for keyed in models {
            let holds = state
                .entries
                .get(&keyed.model)
                .is_some_and(|e| e.holds(&keyed.key));
            if holds {
                continue;
            }
            waiting.push(keyed.model.clone());
            if state.running.contains(&keyed.model) {
                continue;
            }
            if state.queued.contains(&keyed.model) {
                // the newest key and context win; urgent work moves up
                if let Some(i) = state
                    .queue
                    .iter()
                    .position(|j| j.keyed.model == keyed.model)
                {
                    if let Some(mut job) = state.queue.remove(i) {
                        job.keyed = keyed;
                        job.compute = compute.clone();
                        if urgent {
                            state.queue.push_front(job);
                        } else {
                            state.queue.insert(i, job);
                        }
                    }
                }
                continue;
            }
            state.queued.insert(keyed.model.clone());
            let job = Job {
                keyed,
                compute: compute.clone(),
            };
            if urgent {
                state.queue.push_front(job);
            } else {
                state.queue.push_back(job);
            }
        }
        let wanted = state.queue.len().min(self.max_workers);
        while state.workers < wanted {
            state.workers += 1;
            let store = self.clone();
            let spawned = std::thread::Builder::new()
                .name("cogitator-lineage".into())
                .spawn(move || store.work());
            if spawned.is_err() {
                state.workers -= 1;
                break;
            }
        }
        waiting
    }

    /// Waits until none of `models` is queued or running, or `deadline`.
    /// Returns those still not done.
    pub fn wait(&self, models: &[String], deadline: Instant) -> Vec<String> {
        let mut state = self.lock();
        loop {
            let left: Vec<String> = models
                .iter()
                .filter(|m| state.queued.contains(*m) || state.running.contains(*m))
                .cloned()
                .collect();
            let now = Instant::now();
            if left.is_empty() || now >= deadline {
                return left;
            }
            state = self
                .changed
                .wait_timeout(state, deadline - now)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }

    pub fn set_live(&self, live: HashSet<String>) {
        self.lock().live = Some(live);
    }

    fn work(self: Arc<Self>) {
        loop {
            let job = {
                let mut state = self.lock();
                match state.queue.pop_front() {
                    Some(job) => {
                        state.queued.remove(&job.keyed.model);
                        state.running.insert(job.keyed.model.clone());
                        job
                    }
                    None => {
                        state.workers -= 1;
                        let last = state.workers == 0 && state.running.is_empty();
                        let save = (last && state.dirty).then(|| self.take_save(&mut state));
                        drop(state);
                        if let Some(entries) = save {
                            persist::save(&self.root, &entries);
                            self.stats.saves.fetch_add(1, Ordering::Relaxed);
                        }
                        return;
                    }
                }
            };
            let span = tracing::debug_span!("lineage model", model = %job.keyed.model);
            let (lineage, compiled) = span.in_scope(|| (job.compute)(&job.keyed.uri));
            self.stats.computed.fetch_add(1, Ordering::Relaxed);
            let mut state = self.lock();
            state.running.remove(&job.keyed.model);
            state.entries.insert(
                job.keyed.model.clone(),
                Entry {
                    base: job.keyed.key.base,
                    compiled: compiled.then_some(job.keyed.key.compiled),
                    lineage: Arc::new(lineage),
                },
            );
            state.dirty = true;
            drop(state);
            self.changed.notify_all();
        }
    }

    /// The lineage of `keyed`'s model: the entry when it holds, else
    /// computed here, on the caller's thread, and kept like a worker's
    /// answer. For callers that need the answer now; [`Self::flush`]
    /// writes what they added.
    pub fn get_or_compute(
        &self,
        keyed: &Keyed,
        compute: impl FnOnce(&str) -> (Option<ModelLineage>, bool),
    ) -> Arc<Option<ModelLineage>> {
        if let Some(lineage) = self.valid(&keyed.model, &keyed.key) {
            return lineage;
        }
        let span = tracing::debug_span!("lineage model", model = %keyed.model);
        let (lineage, compiled) = span.in_scope(|| compute(&keyed.uri));
        self.stats.computed.fetch_add(1, Ordering::Relaxed);
        let lineage = Arc::new(lineage);
        let mut state = self.lock();
        state.entries.insert(
            keyed.model.clone(),
            Entry {
                base: keyed.key.base,
                compiled: compiled.then_some(keyed.key.compiled),
                lineage: lineage.clone(),
            },
        );
        state.dirty = true;
        drop(state);
        self.changed.notify_all();
        lineage
    }

    /// Writes the store now when it changed and no worker runs (a worker
    /// that is still busy writes it when its queue runs dry).
    pub fn flush(&self) {
        let mut state = self.lock();
        let idle = state.workers == 0 && state.running.is_empty();
        let save = (idle && state.dirty).then(|| self.take_save(&mut state));
        drop(state);
        if let Some(entries) = save {
            persist::save(&self.root, &entries);
            self.stats.saves.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// The entries to write, models that left the project dropped.
    fn take_save(&self, state: &mut State) -> Vec<(String, Entry)> {
        state.dirty = false;
        if let Some(live) = &state.live {
            let live = live.clone();
            state.entries.retain(|model, _| live.contains(model));
        }
        let mut entries: Vec<(String, Entry)> = state
            .entries
            .iter()
            .map(|(m, e)| (m.clone(), e.clone()))
            .collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        entries
    }

    pub fn entries(&self) -> usize {
        self.lock().entries.len()
    }

    pub fn pending(&self) -> usize {
        let state = self.lock();
        state.queue.len() + state.running.len()
    }

    /// Forgets every entry, in memory and on disk.
    pub fn clear(&self) {
        let mut state = self.lock();
        state.entries.clear();
        state.dirty = false;
        drop(state);
        persist::remove(&self.root);
    }
}
