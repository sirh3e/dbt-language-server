//! Column profiles (dbt.profileColumn): filled by the background job,
//! read by the hover. Bounded: the oldest entry goes past `MAX`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use cogitator_analysis::features::profile::{ColumnProfile, ProfileKey};

use super::DerivedCache;

const MAX: usize = 256;

#[derive(Default)]
pub struct ProfileSlot {
    /// key → (insertion order, profile).
    map: Mutex<HashMap<ProfileKey, (u64, Arc<ColumnProfile>)>>,
    next: std::sync::atomic::AtomicU64,
}

impl DerivedCache {
    /// The cached profile of `key`, when it is current for `stamp` (the
    /// last run's `generated_at`, empty without one).
    pub fn column_profile(&self, key: &ProfileKey, stamp: &str) -> Option<Arc<ColumnProfile>> {
        let map = self
            .column_profiles
            .map
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        map.get(key)
            .filter(|(_, p)| p.stamp == stamp)
            .map(|(_, p)| p.clone())
    }

    pub fn store_column_profile(&self, key: ProfileKey, profile: ColumnProfile) {
        let order = self
            .column_profiles
            .next
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let mut map = self
            .column_profiles
            .map
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        map.insert(key, (order, Arc::new(profile)));
        if map.len() > MAX {
            if let Some(oldest) = map
                .iter()
                .min_by_key(|(_, (o, _))| *o)
                .map(|(k, _)| k.clone())
            {
                map.remove(&oldest);
            }
        }
    }
}
