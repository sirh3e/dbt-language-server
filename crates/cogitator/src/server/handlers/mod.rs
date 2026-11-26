//! Pure message handlers (05 §6.2): `fn(&Snapshot, …) -> Reply` — every
//! effectful or offloadable handler lives here, producing ordered
//! effects plus a response that the shell transmits. Sub-millisecond
//! query handlers stay on the dispatcher's generic decode→feature→send
//! path; their logic is already pure in `cogitator_analysis::features`.

pub mod commands;
pub mod cost;
pub mod diff_at;
pub mod doctor;
pub mod impact;
pub mod incremental;
pub mod lineage;
pub mod lineage_graph;
pub mod lineage_hints;
pub mod lineage_site;
pub mod navigation;
pub mod plan;
mod plan_compiled;
pub mod plan_hints;
pub mod preview_cte;
pub mod profiles;
pub mod search;
