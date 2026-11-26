//! The delta filter text `incr.could_be_incremental`'s rewrite writes,
//! shared with the editor's `is_incremental()` snippet so both say it the
//! same way: `col > (select max(newest) from {{ this }})`.

/// `(select max(newest) from {{ this }})`: the newest value built so far.
pub fn this_max(newest: &str) -> String {
    format!("(select max({newest}) from {{{{ this }}}})")
}

/// `col > (select max(newest) from {{ this }})`: rows strictly after
/// the newest built, as append adds what it is given.
pub fn newer_than_built(column: &str, newest: &str) -> String {
    format!("{column} > {}", this_max(newest))
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_filter_reads_this() {
        assert_eq!(
            super::newer_than_built("o.ts", "ts"),
            "o.ts > (select max(ts) from {{ this }})"
        );
    }
}
