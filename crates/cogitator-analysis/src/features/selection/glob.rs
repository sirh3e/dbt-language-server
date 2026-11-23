//! Python's `fnmatch.fnmatchcase`, which dbt matches selector values
//! with: `*` any run, `?` one character, `[seq]` / `[!seq]` a set (with
//! ranges). Case-sensitive, as dbt is.

/// Does `text` match the glob `pattern`?
pub fn matches(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    match_from(&p, &t)
}

/// Does `value` hold a glob character (dbt's wildcard test)?
pub fn is_glob(value: &str) -> bool {
    value.contains(['*', '?', '[', ']'])
}

fn match_from(p: &[char], t: &[char]) -> bool {
    // iterative with backtracking to the last `*`
    let (mut pi, mut ti) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None;
    while ti < t.len() {
        if pi < p.len() {
            match p[pi] {
                '*' => {
                    star = Some((pi, ti));
                    pi += 1;
                    continue;
                }
                '?' => {
                    pi += 1;
                    ti += 1;
                    continue;
                }
                '[' => {
                    if let Some((hit, next)) = set(p, pi, t[ti]) {
                        if hit {
                            pi = next;
                            ti += 1;
                            continue;
                        }
                    } else if t[ti] == '[' {
                        // an unclosed `[` is a literal, as in fnmatch
                        pi += 1;
                        ti += 1;
                        continue;
                    }
                }
                c if c == t[ti] => {
                    pi += 1;
                    ti += 1;
                    continue;
                }
                _ => {}
            }
        }
        match star {
            Some((sp, st)) => {
                pi = sp + 1;
                ti = st + 1;
                star = Some((sp, st + 1));
            }
            None => return false,
        }
    }
    p[pi..].iter().all(|&c| c == '*')
}

/// The set at `p[start] == '['`: whether `c` is in it, and the index
/// after its `]`. None when the set never closes.
fn set(p: &[char], start: usize, c: char) -> Option<(bool, usize)> {
    let mut i = start + 1;
    let negate = matches!(p.get(i), Some('!'));
    if negate {
        i += 1;
    }
    let mut hit = false;
    let mut first = true;
    while i < p.len() {
        if p[i] == ']' && !first {
            return Some((hit != negate, i + 1));
        }
        first = false;
        if i + 2 < p.len() && p[i + 1] == '-' && p[i + 2] != ']' {
            if p[i] <= c && c <= p[i + 2] {
                hit = true;
            }
            i += 3;
        } else {
            if p[i] == c {
                hit = true;
            }
            i += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("stg_*", "stg_orders", true)]
    #[case("stg_*", "fct_orders", false)]
    #[case("*", "", true)]
    #[case("*orders", "stg_orders", true)]
    #[case("a?c", "abc", true)]
    #[case("a?c", "ac", false)]
    #[case("[abc]x", "bx", true)]
    #[case("[!abc]x", "bx", false)]
    #[case("[a-c]x", "cx", true)]
    #[case("v[0-9]", "v7", true)]
    #[case("[x", "[x", true)]
    #[case("Orders", "orders", false)]
    #[case("a*b*c", "aXXbYYc", true)]
    #[case("a*b*c", "aXXbYY", false)]
    fn fnmatchcase(#[case] pattern: &str, #[case] text: &str, #[case] expected: bool) {
        assert_eq!(matches(pattern, text), expected, "{pattern} ~ {text}");
    }
}
