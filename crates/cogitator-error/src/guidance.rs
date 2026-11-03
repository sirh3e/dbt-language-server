//! Elm/rustc-grade user guidance (05 §2): every user-facing failure is
//! ONE `Guidance` value — what happened, where, why, what might have
//! been meant, and the concrete next step — rendered canonically for
//! window/showMessage and for diagnostics. No ad-hoc message strings.

/// A structured user-facing message.
///
/// Rendered shape (`to_message`):
/// ```text
/// Target 'prdo' not found in profile 'jaffle_shop'.
///   in: /home/me/.dbt/profiles.yml
/// Reason: the profile defines targets 'dev' and 'prod'.
/// Did you mean: prod?
/// Solution: run dbt.setTarget with {"target": "prod"}.
/// ```
#[derive(Debug, Clone)]
pub struct Guidance {
    /// What happened — one sentence that names the thing.
    pub headline: String,
    /// Where — a path, or `path:line`.
    pub location: Option<String>,
    /// Why — one sentence of evidence.
    pub reason: Option<String>,
    /// Did-you-mean candidates, closest first (possibly empty).
    pub suggestions: Vec<String>,
    /// The concrete next step — imperative.
    pub solution: String,
}

impl Guidance {
    pub fn new(headline: impl Into<String>) -> Self {
        Guidance {
            headline: headline.into(),
            location: None,
            reason: None,
            suggestions: Vec::new(),
            solution: String::new(),
        }
    }

    pub fn at(mut self, location: impl Into<String>) -> Self {
        self.location = Some(location.into());
        self
    }

    pub fn because(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    pub fn suggest<I: IntoIterator<Item = String>>(mut self, suggestions: I) -> Self {
        self.suggestions = suggestions.into_iter().collect();
        self
    }

    pub fn fix(mut self, solution: impl Into<String>) -> Self {
        self.solution = solution.into();
        self
    }

    /// The multi-line window/showMessage body.
    pub fn to_message(&self) -> String {
        let mut out = self.headline.clone();
        if let Some(location) = &self.location {
            out.push_str("\n  in: ");
            out.push_str(location);
        }
        if let Some(reason) = &self.reason {
            out.push_str("\nReason: ");
            out.push_str(reason);
        }
        if !self.suggestions.is_empty() {
            out.push_str("\nDid you mean: ");
            out.push_str(&self.suggestions.join(", "));
            out.push('?');
        }
        if !self.solution.is_empty() {
            out.push_str("\nSolution: ");
            out.push_str(&self.solution);
        }
        out
    }

    /// The compact one-or-two-clause diagnostic text.
    pub fn to_diagnostic(&self) -> String {
        let mut out = self.headline.clone();
        if !self.suggestions.is_empty() {
            // diagnostics read inline: strip the headline's final period
            if out.ends_with('.') {
                out.pop();
            }
            out.push_str(" — did you mean '");
            out.push_str(&self.suggestions[0]);
            out.push_str("'?");
        }
        out
    }
}

/// The closest candidates to `input` by Levenshtein distance: at most
/// three, within `max(1, len/3)` edits, sorted by (distance, name).
pub fn closest_matches<'a>(
    input: &str,
    candidates: impl IntoIterator<Item = &'a str>,
) -> Vec<String> {
    let budget = (input.chars().count() / 3).max(1);
    let mut scored: Vec<(usize, &str)> = candidates
        .into_iter()
        .filter(|c| !c.is_empty() && *c != input)
        .filter_map(|c| {
            let d = levenshtein(input, c);
            (d <= budget).then_some((d, c))
        })
        .collect();
    scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(b.1)));
    scored.truncate(3);
    scored.into_iter().map(|(_, c)| c.to_string()).collect()
}

/// Optimal-string-alignment distance over chars (Levenshtein plus
/// adjacent transpositions — 'prdo' is ONE edit from 'prod', the
/// canonical typo). Small inputs only (names, commands).
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    let mut prev_prev = vec![0usize; b.len() + 1];
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut current = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        current[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitution = prev[j] + usize::from(ca != cb);
            let mut best = substitution.min(prev[j + 1] + 1).min(current[j] + 1);
            if i > 0 && j > 0 && a[i] == b[j - 1] && a[i - 1] == b[j] {
                best = best.min(prev_prev[j - 1] + 1);
            }
            current[j + 1] = best;
        }
        std::mem::swap(&mut prev_prev, &mut prev);
        std::mem::swap(&mut prev, &mut current);
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_rendering() {
        let g = Guidance::new("Target 'prdo' not found in profile 'jaffle_shop'.")
            .at("/home/me/.dbt/profiles.yml")
            .because("the profile defines targets 'dev' and 'prod'.")
            .suggest(vec!["prod".to_string()])
            .fix("run dbt.setTarget with {\"target\": \"prod\"}.");
        assert_eq!(
            g.to_message(),
            "Target 'prdo' not found in profile 'jaffle_shop'.\n\
             \u{20}\u{20}in: /home/me/.dbt/profiles.yml\n\
             Reason: the profile defines targets 'dev' and 'prod'.\n\
             Did you mean: prod?\n\
             Solution: run dbt.setTarget with {\"target\": \"prod\"}."
        );
    }

    #[test]
    fn optional_parts_disappear() {
        let g = Guidance::new("No profiles directory given.").fix("pass {\"dir\": \"…\"}.");
        assert_eq!(
            g.to_message(),
            "No profiles directory given.\nSolution: pass {\"dir\": \"…\"}."
        );
        assert_eq!(g.to_diagnostic(), "No profiles directory given.");
    }

    #[test]
    fn diagnostic_form_carries_the_best_suggestion() {
        let g = Guidance::new("model 'stg_ordrs' not found in the project.")
            .suggest(vec!["stg_orders".to_string(), "stg_payments".to_string()]);
        assert_eq!(
            g.to_diagnostic(),
            "model 'stg_ordrs' not found in the project — did you mean 'stg_orders'?"
        );
    }

    #[test]
    fn closest_matches_typos_and_budget() {
        let candidates = ["prod", "dev", "staging"];
        assert_eq!(closest_matches("prdo", candidates), vec!["prod"]);
        assert_eq!(closest_matches("dev", candidates), Vec::<String>::new());
        // way off: nothing within the budget
        assert_eq!(
            closest_matches("warehouse", candidates),
            Vec::<String>::new()
        );
        // adjacent transposition is one edit, so short typos still match
        assert_eq!(
            closest_matches("dve", candidates.iter().copied()),
            vec!["dev"]
        );
        assert_eq!(closest_matches("dev2", candidates), vec!["dev"]);
    }

    #[test]
    fn closest_matches_sorts_and_caps() {
        let candidates = ["customers", "customer_x", "customera", "customerb"];
        let got = closest_matches("customer", candidates);
        assert_eq!(got.len(), 3);
        assert!(got.contains(&"customers".to_string()));
    }
}
