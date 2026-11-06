//! Multi-root workspaces (01 §3 addendum): the FIRST workspace folder
//! stays the primary root — its context lives on the watch channel and
//! feeds every project-wide surface (workspace/symbol, workspace
//! diagnostics, the config watcher, dbt.status). Every OTHER dbt
//! project — further workspaceFolders, or a file opened from a foreign
//! project — gets its own context here, built lazily by the refresh
//! task and picked per URI by the longest project-root prefix.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, RwLock};

use cogitator_analysis::DbtContext;

#[derive(Default)]
pub struct ExtraContexts {
    /// project_root (display string) → its latest context.
    map: RwLock<HashMap<String, Arc<DbtContext>>>,
}

impl ExtraContexts {
    pub fn insert(&self, root: String, ctx: Arc<DbtContext>) {
        self.map
            .write()
            .expect("extra contexts lock")
            .insert(root, ctx);
    }

    /// The context whose project root contains `uri`, longest root
    /// winning — compared as `Path` components (08 §1), never strings.
    pub fn for_uri(&self, uri: &str) -> Option<(String, Arc<DbtContext>)> {
        let path = cogitator_lsp::uri_to_fs_path(uri)?;
        let path = Path::new(&path);
        let map = self.map.read().expect("extra contexts lock");
        map.iter()
            .filter(|(root, _)| path.starts_with(Path::new(root)))
            .max_by_key(|(root, _)| Path::new(root).components().count())
            .map(|(root, ctx)| (root.clone(), ctx.clone()))
    }

    /// Known secondary roots (dbt.status reporting).
    pub fn roots(&self) -> Vec<String> {
        let mut roots: Vec<String> = self
            .map
            .read()
            .expect("extra contexts lock")
            .keys()
            .cloned()
            .collect();
        roots.sort();
        roots
    }
}

/// Does `root` (a project root as display string) contain the uri's
/// file, component-wise?
pub fn root_contains(root: &str, uri: &str) -> bool {
    if root.is_empty() {
        return false;
    }
    cogitator_lsp::uri_to_fs_path(uri)
        .is_some_and(|path| Path::new(&path).starts_with(Path::new(root)))
}
