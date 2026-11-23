//! Python's `fnmatch.fnmatchcase`, which dbt's selector methods match
//! values with: `*` any run, `?` one character, `[abc]`/`[!abc]`/`[a-z]`
//! a class. Case-sensitive, as on POSIX (dbt does not fold case).

/// Whether `pattern` holds a glob character at all.
pub fn is_glob(pattern: &str) -> bool {
    pattern.contains(['*', '?', '['])
}

/// `fnmatchcase(text, pattern)`.
pub fn matches(text: &str, pattern: &str) -> bool {
    let text: Vec<char> = text.chars().collect();
    let pattern: Vec<char> = pattern.chars().collect();
    match_from(&text, &pattern)
}

fn match_from(text: &[char], pattern: &[char]) -> bool {
    let Some((&first, rest)) = pattern.split_first() else {
        return text.is_empty();
    };
    match first {
        '*' => (0..=text.len()).any(|skip| match_from(&text[skip..], rest)),
        '?' => !text.is_empty() && match_from(&text[1..], rest),
        '[' => match class(rest) {
            Some((set, after)) => text
                .split_first()
                .is_some_and(|(c, tail)| set.contains(*c) && match_from(tail, after)),
            // an unclosed `[` is a literal, as in Python
            None => text
                .split_first()
                .is_some_and(|(c, tail)| *c == '[' && match_from(tail, rest)),
        },
        c => text
            .split_first()
            .is_some_and(|(t, tail)| *t == c && match_from(tail, rest)),
    }
}

struct Class {
    negated: bool,
    items: Vec<(char, char)>,
}

impl Class {
    fn contains(&self, c: char) -> bool {
        self.items.iter().any(|(lo, hi)| (*lo..=*hi).contains(&c)) != self.negated
    }
}

/// The class after a `[`: its members and the pattern after its `]`.
fn class(pattern: &[char]) -> Option<(Class, &[char])> {
    let (negated, mut i) = match pattern.first() {
        Some('!') => (true, 1),
        _ => (false, 0),
    };
    let mut items = Vec::new();
    // a `]` right after the opening is a member, as in Python
    let start = i;
    while i < pattern.len() {
        let c = pattern[i];
        if c == ']' && i > start {
            return Some((Class { negated, items }, &pattern[i + 1..]));
        }
        if pattern.get(i + 1) == Some(&'-') && pattern.get(i + 2).is_some_and(|h| *h != ']') {
            items.push((c, pattern[i + 2]));
            i += 3;
        } else {
            items.push((c, c));
            i += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fnmatchcase_cases() {
        let cases: &[(&str, &str, bool)] = &[
            ("customers", "customers", true),
            ("customers", "cust*", true),
            ("customers", "*mers", true),
            ("customers", "c?stomers", true),
            ("customers", "Customers", false),
            ("customers", "orders", false),
            ("stg_orders", "stg_[ao]rders", true),
            ("stg_orders", "stg_[!o]rders", false),
            ("v2", "v[0-9]", true),
            ("a]", "a[]]", true),
            ("[x", "[x", true),
            ("", "*", true),
            ("abc", "*", true),
            ("", "?", false),
        ];
        for (text, pattern, expected) in cases {
            assert_eq!(matches(text, pattern), *expected, "{text} ~ {pattern}");
        }
    }

    #[test]
    fn glob_detection() {
        assert!(is_glob("stg_*"));
        assert!(!is_glob("stg_orders"));
    }
}
