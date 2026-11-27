//! A column profile as hover markdown, with a block sparkline of the
//! top values' counts.

use super::ColumnProfile;

const BLOCKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// One block per count, scaled to the largest (▁ the smallest share).
pub fn sparkline(counts: &[u64]) -> String {
    let Some(&max) = counts.iter().max() else {
        return String::new();
    };
    counts
        .iter()
        .map(|&count| {
            if max == 0 {
                BLOCKS[0]
            } else {
                let step = (count as f64 / max as f64 * (BLOCKS.len() - 1) as f64).round();
                BLOCKS[step as usize]
            }
        })
        .collect()
}

fn percent(share: f64) -> String {
    format!("{:.1}%", share * 100.0)
}

pub fn hover_markdown(profile: &ColumnProfile) -> String {
    let mut out = format!(
        "**Profile of `{}`** · {} rows sampled from `{}`\n\n",
        profile.column, profile.rows, profile.relation
    );
    out.push_str(&format!(
        "distinct {} · null {}",
        profile.distinct,
        percent(profile.null_rate)
    ));
    if let (Some(min), Some(max)) = (&profile.min, &profile.max) {
        out.push_str(&format!(" · min `{min}` · max `{max}`"));
    }
    out.push('\n');
    if !profile.top.is_empty() {
        let counts: Vec<u64> = profile.top.iter().map(|t| t.count).collect();
        out.push_str(&format!("\ntop values {}\n", sparkline(&counts)));
        for top in &profile.top {
            out.push_str(&format!("- `{}` × {}\n", top.value, top.count));
        }
    }
    out.push_str(&format!(
        "\n_{} · {}_",
        profile.engine,
        profile.stamp_note()
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparkline_scales_to_the_largest() {
        assert_eq!(sparkline(&[10, 5, 1, 0]), "█▅▂▁");
        assert_eq!(sparkline(&[]), "");
        assert_eq!(sparkline(&[0, 0]), "▁▁");
    }
}
