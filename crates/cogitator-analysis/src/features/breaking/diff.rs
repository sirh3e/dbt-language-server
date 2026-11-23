//! Current columns against built ones, by name (case-insensitive):
//! removed, added, retyped, and "possibly renamed" — a removed and an
//! added column with similar names, paired once each. A pair is only
//! ever a hint: the removal still counts as breaking.

use serde::Serialize;

use super::{BaselineColumn, CurrentColumn};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaDiff {
    pub removed: Vec<RemovedColumn>,
    /// Informational: new columns break nothing.
    pub added: Vec<CurrentColumn>,
    pub retyped: Vec<TypeChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemovedColumn {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_type: Option<String>,
    /// An added column whose name looks like this one's — never asserted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub possibly_renamed_to: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeChange {
    pub name: String,
    pub from: String,
    pub to: String,
}

/// Similarity at or above which a removed/added pair is offered as a
/// possible rename.
const RENAME_SIMILARITY: f64 = 0.5;

pub fn diff(current: &[CurrentColumn], built: &[BaselineColumn]) -> SchemaDiff {
    let find = |name: &str| current.iter().find(|c| c.name.eq_ignore_ascii_case(name));
    let mut out = SchemaDiff::default();
    for column in built {
        match find(&column.name) {
            None => out.removed.push(RemovedColumn {
                name: column.name.clone(),
                data_type: column.data_type.clone(),
                possibly_renamed_to: None,
            }),
            Some(now) => {
                if super::types::changed(now.data_type.as_deref(), column.data_type.as_deref()) {
                    out.retyped.push(TypeChange {
                        name: column.name.clone(),
                        from: column.data_type.clone().unwrap_or_default(),
                        to: now.data_type.clone().unwrap_or_default(),
                    });
                }
            }
        }
    }
    out.added = current
        .iter()
        .filter(|c| !built.iter().any(|b| b.name.eq_ignore_ascii_case(&c.name)))
        .cloned()
        .collect();
    pair_renames(&mut out.removed, &out.added);
    out
}

/// Greedy one-to-one pairing, best similarity first, ties by name order.
fn pair_renames(removed: &mut [RemovedColumn], added: &[CurrentColumn]) {
    let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
    for (r, old) in removed.iter().enumerate() {
        for (a, new) in added.iter().enumerate() {
            let score = similarity(&old.name, &new.name);
            if score >= RENAME_SIMILARITY {
                pairs.push((score, r, a));
            }
        }
    }
    pairs.sort_by(|x, y| y.0.total_cmp(&x.0).then((x.1, x.2).cmp(&(y.1, y.2))));
    let mut used = vec![false; added.len()];
    for (_, r, a) in pairs {
        if removed[r].possibly_renamed_to.is_some() || used[a] {
            continue;
        }
        used[a] = true;
        removed[r].possibly_renamed_to = Some(added[a].name.clone());
    }
}

/// 0..=1: the better of normalized edit distance and the share of
/// `_`-separated words the two names have in common.
pub(super) fn similarity(a: &str, b: &str) -> f64 {
    let a = a.to_ascii_lowercase();
    let b = b.to_ascii_lowercase();
    let longest = a.chars().count().max(b.chars().count());
    if longest == 0 {
        return 0.0;
    }
    let edit = 1.0 - levenshtein(&a, &b) as f64 / longest as f64;
    let words = |s: &str| -> Vec<String> {
        s.split('_')
            .filter(|w| !w.is_empty())
            .map(str::to_string)
            .collect()
    };
    let (wa, wb) = (words(&a), words(&b));
    let shared = wa.iter().filter(|w| w.len() >= 3 && wb.contains(w)).count();
    let most = wa.len().max(wb.len()).max(1);
    edit.max(shared as f64 / most as f64)
}

fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitute = previous + usize::from(ca != *cb);
            previous = row[j + 1];
            row[j + 1] = substitute.min(row[j] + 1).min(row[j + 1] + 1);
        }
    }
    row[b.len()]
}
