use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use cogitator_analysis::lineage::{ColumnLineage, ModelLineage};

use super::stamps::{Keyed, ModelKey};
use super::{persist, Compute, Store};

fn root(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("cogitator-lineage-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn keyed(model: &str, base: u64) -> Keyed {
    Keyed {
        model: model.to_string(),
        uri: format!("file:///{model}.sql"),
        key: ModelKey { base, compiled: 0 },
    }
}

/// A compute that records which uris it saw.
fn counting() -> (Compute, Arc<Mutex<Vec<String>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let compute: Compute = Arc::new(move |uri: &str| {
        log.lock().unwrap().push(uri.to_string());
        let lineage = ModelLineage {
            columns: vec![ColumnLineage {
                name: "id".into(),
                sources: Vec::new(),
                direct: false,
                derived: false,
                end: None,
            }],
            origin: None,
        };
        (Some(lineage), false)
    });
    (compute, seen)
}

fn settle(store: &Store) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while store.pending() > 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn one_changed_model_is_the_only_one_computed_again() {
    let store = Arc::new(Store::open(root("invalidate"), 4));
    let (compute, seen) = counting();
    let models: Vec<Keyed> = ["a", "b", "c"].iter().map(|m| keyed(m, 1)).collect();
    let waiting = store.ensure(models.clone(), &compute, true);
    assert!(store
        .wait(&waiting, Instant::now() + Duration::from_secs(10))
        .is_empty());
    assert_eq!(seen.lock().unwrap().len(), 3);

    let again = store.ensure(models.clone(), &compute, true);
    assert!(again.is_empty(), "nothing changed, nothing queued");
    let changed = vec![keyed("a", 1), keyed("b", 2), keyed("c", 1)];
    let waiting = store.ensure(changed, &compute, true);
    assert_eq!(waiting, ["b"]);
    store.wait(&waiting, Instant::now() + Duration::from_secs(10));
    let seen = seen.lock().unwrap();
    assert_eq!(seen.len(), 4);
    assert_eq!(seen[3], "file:///b.sql");
    assert!(store
        .valid(
            "b",
            &ModelKey {
                base: 2,
                compiled: 0
            }
        )
        .is_some());
    assert!(store
        .valid(
            "b",
            &ModelKey {
                base: 1,
                compiled: 0
            }
        )
        .is_none());
}

#[test]
fn a_compiled_answer_also_keys_on_the_compiled_file() {
    let store = Arc::new(Store::open(root("compiled"), 1));
    let routes = Arc::new(HashMap::from([("file:///t.sql", true)]));
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let compute: Compute = Arc::new(move |uri: &str| {
        count.fetch_add(1, Ordering::SeqCst);
        (None, routes.get(uri).copied().unwrap_or(false))
    });
    let key = |base, compiled| Keyed {
        key: ModelKey { base, compiled },
        ..keyed("t", 0)
    };
    let plain = |base, compiled| Keyed {
        key: ModelKey { base, compiled },
        ..keyed("p", 0)
    };
    let w = store.ensure([key(1, 1), plain(1, 1)], &compute, true);
    store.wait(&w, Instant::now() + Duration::from_secs(10));
    // a new compiled file: the template is computed again, the plain model not
    let w = store.ensure([key(1, 2), plain(1, 2)], &compute, true);
    assert_eq!(w, ["t"]);
    store.wait(&w, Instant::now() + Duration::from_secs(10));
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

#[test]
fn the_store_is_written_when_its_queue_runs_dry_and_read_back() {
    let dir = root("store-disk");
    let store = Arc::new(Store::open(dir.clone(), 2));
    let (compute, _) = counting();
    store.set_live(["a".to_string(), "b".to_string()].into());
    let w = store.ensure([keyed("a", 7), keyed("b", 8)], &compute, true);
    store.wait(&w, Instant::now() + Duration::from_secs(10));
    settle(&store);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !persist::cache_path(&dir).is_file() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    let reopened = Store::open(dir.clone(), 2);
    assert_eq!(reopened.entries(), 2, "read from disk");
    let lineage = reopened
        .valid(
            "a",
            &ModelKey {
                base: 7,
                compiled: 0,
            },
        )
        .expect("a holds after a restart");
    assert_eq!((*lineage).as_ref().unwrap().columns[0].name, "id");
    reopened.clear();
    assert!(
        !persist::cache_path(&dir).exists(),
        "clear forgets the file"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
