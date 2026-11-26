//! Case tables for the boolean structure of a query.

use super::super::query::parse;
use super::{words, Expr};

use Expr::{And, Leaf, Not, Or};

fn not(e: Expr) -> Expr {
    Not(Box::new(e))
}

#[test]
fn precedence_and_grouping() {
    for (text, want) in [
        ("reads a", Leaf(0)),
        ("reads a and reads b", And(vec![Leaf(0), Leaf(1)])),
        // `and` binds tighter than `or`
        (
            "reads a or reads b and reads c",
            Or(vec![Leaf(0), And(vec![Leaf(1), Leaf(2)])]),
        ),
        (
            "reads a and reads b or reads c",
            Or(vec![And(vec![Leaf(0), Leaf(1)]), Leaf(2)]),
        ),
        // `not` binds tighter than `and`
        ("not reads a and reads b", And(vec![not(Leaf(0)), Leaf(1)])),
        ("not not reads a", not(not(Leaf(0)))),
        // parentheses group, touching words or not
        (
            "(reads a or reads b) and reads c",
            And(vec![Or(vec![Leaf(0), Leaf(1)]), Leaf(2)]),
        ),
        (
            "not ( reads a or reads b )",
            not(Or(vec![Leaf(0), Leaf(1)])),
        ),
        ("((reads a))", Leaf(0)),
        ("READS a OR NOT reads b", Or(vec![Leaf(0), not(Leaf(1))])),
    ] {
        assert_eq!(parse(text).map(|q| q.expr).ok(), Some(want), "{text}");
    }
}

#[test]
fn a_clause_reads_its_words_up_to_an_operator() {
    let q = parse("joins stg_orders on customer_id or select-star from source").unwrap();
    assert_eq!(q.clauses.len(), 2);
    assert_eq!(q.clauses[0].describe(), "joins stg_orders on customer_id");
    assert_eq!(q.clauses[1].describe(), "select-star from source");
}

#[test]
fn parentheses_are_words_of_their_own() {
    assert_eq!(
        words("(reads a) and ((not reads b))"),
        ["(", "reads", "a", ")", "and", "(", "(", "not", "reads", "b", ")", ")"]
    );
}

#[test]
fn refusals() {
    for (text, why) in [
        ("not", "nothing after it"),
        ("reads a and not", "nothing after it"),
        ("reads a or", "empty"),
        ("or reads a", "empty"),
        ("reads a and or reads b", "empty"),
        ("(reads a", "not closed"),
        ("reads a)", "closes nothing"),
        ("()", "empty"),
        ("(reads a or reads b))", "closes nothing"),
    ] {
        let err = parse(text).unwrap_err().to_message();
        assert!(err.contains(why), "{text}: {err}");
    }
}

#[test]
fn describe_writes_parentheses_only_where_needed() {
    for (text, back) in [
        (
            "reads a or reads b and reads c",
            "reads a or reads b and reads c",
        ),
        (
            "(reads a or reads b) and reads c",
            "(reads a or reads b) and reads c",
        ),
        ("not (reads a and reads b)", "not (reads a and reads b)"),
        ("((not reads a)) OR reads b", "not reads a or reads b"),
        ("not not reads a", "not not reads a"),
    ] {
        assert_eq!(parse(text).unwrap().describe(), back, "{text}");
    }
}

#[test]
fn holds_and_witnesses() {
    let q = parse("reads a or reads b and not reads c").unwrap();
    // a found: holds through the first branch alone
    assert!(q.expr.holds(&[true, false, false]));
    assert_eq!(q.expr.witnesses(&[true, false, false]), [0]);
    // b found, c not: the second branch, whose `not` shows no site
    assert!(q.expr.holds(&[false, true, false]));
    assert_eq!(q.expr.witnesses(&[false, true, false]), [1]);
    // b and c found: the negation fails
    assert!(!q.expr.holds(&[false, true, true]));
    // a `not` alone holds with no witness
    let only_not = parse("not reads a").unwrap();
    assert!(only_not.expr.holds(&[false]));
    assert!(only_not.expr.witnesses(&[false]).is_empty());
}
