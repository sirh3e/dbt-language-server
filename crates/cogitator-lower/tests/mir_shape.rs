//! HIR→MIR: CTE sharing, correlation, holes, open schemas, set ops, caps
//! and totality on bad input.

mod common;

use cogitator_ir::common::{BinaryOp, HoleKind, JoinKind, SetOpKind};
use cogitator_ir::hir::{Body, ExprKind, FromKind, GroupBy, RelTarget, Resolution};
use cogitator_ir::mir::{print_mir, HirNode, Origin, RelOp, ScalarKind};
use cogitator_ir::{ColType, DegradationKind, Fidelity, Idx, Invariant, Nullability, SqlType};
use cogitator_lower::lower_to_mir;
use common::*;

#[test]
fn cte_references_share_only_through_cte_ref() {
    let mut b = builder();
    let t = b.base_model(SPAN, "t", None, &["a"]);
    let inner = select(&mut b, vec![], vec![star()], vec![t], |_| {});
    let cte = b.cte(SPAN, "c", inner);
    let c1 = b.base_cte(SPAN, cte, Some("x"), &["a"]);
    let c2 = b.base_cte(SPAN, cte, Some("y"), &["a"]);
    let joined = b.join(
        SPAN,
        JoinKind::Cross,
        c1,
        c2,
        cogitator_ir::hir::JoinCond::None,
    );
    let root = select(&mut b, vec![cte], vec![star()], vec![joined], |_| {});
    let mir = clean(&b.finish(root));
    assert_tree(&mir);
    assert_eq!(mir.ctes.len(), 1);
    assert_eq!(count_ops(&mir, |op| matches!(op, RelOp::CteRef { .. })), 2);
    insta::assert_snapshot!(print_mir(&mir));
}

#[test]
fn cte_column_list_renames_its_output() {
    let mut b = builder();
    let t = b.base_model(SPAN, "t", None, &["a"]);
    let inner = select(&mut b, vec![], vec![star()], vec![t], |_| {});
    let cte = b.cte(SPAN, "c", inner);
    let renamed = b.ident(SPAN, "renamed");
    b.hir.ctes[cte].columns = vec![renamed];
    let from = b.base_cte(SPAN, cte, None, &["renamed"]);
    let root = select(&mut b, vec![cte], vec![star()], vec![from], |_| {});
    let printed = print_mir(&clean(&b.finish(root)));
    assert!(printed.contains("renamed#"), "{printed}");
}

#[test]
fn provenance_reaches_the_hir_for_every_node() {
    let mut b = builder();
    let t = b.base_model(SPAN, "t", None, &["a"]);
    let a = b.column(SPAN, "a", t, 0);
    let items = vec![item(a, "", &mut b)];
    let root = select(&mut b, vec![], items, vec![t], |_| {});
    let mir = clean(&b.finish(root));
    for id in mir.reachable() {
        let origin = &mir.prov.entries[mir.rels[id].prov];
        assert!(matches!(origin, Origin::Hir(r) if r.node != HirNode::None));
    }
}

#[test]
fn correlated_subquery_declares_its_outer_columns() {
    let mut b = builder();
    let outer = b.base_model(SPAN, "o", None, &["a"]);
    let inner = b.base_model(SPAN, "i", None, &["b"]);
    let ib = b.column(SPAN, "b", inner, 0);
    let oa = outer_col(&mut b, "a", outer, 0);
    let cond = b.binary(SPAN, BinaryOp::Eq, ib, oa);
    let sub = select(&mut b, vec![], vec![star()], vec![inner], |s| {
        s.filter = Some(cond)
    });
    let exists = expr(&mut b, ExprKind::Exists(sub));
    let root = select(&mut b, vec![], vec![star()], vec![outer], |s| {
        s.filter = Some(exists)
    });
    let mir = clean(&b.finish(root));
    let correlated = mir.scalars.values().find_map(|s| match &s.kind {
        ScalarKind::Subquery { correlated, .. } => Some(correlated.len()),
        _ => None,
    });
    assert_eq!(correlated, Some(1));
    insta::assert_snapshot!(print_mir(&mir));
}

#[test]
fn doubly_nested_correlation_is_declared_at_every_level() {
    let mut b = builder();
    let outer = b.base_model(SPAN, "o", None, &["a"]);
    let middle = b.base_model(SPAN, "m", None, &["b"]);
    let inner = b.base_model(SPAN, "i", None, &["c"]);
    let ic = b.column(SPAN, "c", inner, 0);
    let oa = outer_col(&mut b, "a", outer, 0);
    let cond = b.binary(SPAN, BinaryOp::Eq, ic, oa);
    let deepest = select(&mut b, vec![], vec![star()], vec![inner], |s| {
        s.filter = Some(cond)
    });
    let exists = expr(&mut b, ExprKind::Exists(deepest));
    let mid = select(&mut b, vec![], vec![star()], vec![middle], |s| {
        s.filter = Some(exists)
    });
    let outer_exists = expr(&mut b, ExprKind::Exists(mid));
    let root = select(&mut b, vec![], vec![star()], vec![outer], |s| {
        s.filter = Some(outer_exists)
    });
    clean(&b.finish(root));
}

#[test]
fn a_correlated_lateral_derived_table_is_opaque() {
    let mut b = builder();
    let t = b.base_model(SPAN, "t", None, &["a"]);
    let a = outer_col(&mut b, "a", t, 0);
    let items = vec![item(a, "x", &mut b)];
    let inner = select(&mut b, vec![], items, vec![], |_| {});
    let lateral = add_from(
        &mut b,
        FromKind::Derived {
            q: inner,
            lateral: true,
            alias: None,
        },
        &["x"],
    );
    let root = select(&mut b, vec![], vec![star()], vec![t, lateral], |_| {});
    let lowered = lower_to_mir(&b.finish(root));
    lowered.assert_clean();
    assert_eq!(lowered.fidelity, Fidelity::Partial);
}

#[test]
fn a_body_hole_is_opaque_and_keeps_the_rest_of_the_plan() {
    let mut b = builder();
    let hole = b.query(SPAN, vec![], Body::Hole(HoleKind::ParseError));
    let cte = b.cte(SPAN, "c", hole);
    let from = b.base_cte(SPAN, cte, None, &[]);
    open(&mut b, from);
    let col = open_col(&mut b, "x", from);
    let items = vec![item(col, "", &mut b)];
    let root = select(&mut b, vec![cte], items, vec![from], |_| {});
    let lowered = lower_to_mir(&b.finish(root));
    lowered.assert_clean();
    assert_eq!(lowered.fidelity, Fidelity::Partial);
    assert!(lowered
        .degraded
        .iter()
        .any(|d| d.kind == DegradationKind::ParseHole));
    insta::assert_snapshot!(print_mir(&lowered.value.expect("a plan")));
}

#[test]
fn an_expression_hole_is_opaque_with_children_retained() {
    let mut b = builder();
    let t = b.base_model(SPAN, "t", None, &["a"]);
    let a = b.column(SPAN, "a", t, 0);
    let opaque = expr(&mut b, ExprKind::Opaque { children: vec![a] });
    let hole = expr(&mut b, ExprKind::Hole);
    let items = vec![item(opaque, "x", &mut b), item(hole, "y", &mut b)];
    let root = select(&mut b, vec![], items, vec![t], |_| {});
    let lowered = lower_to_mir(&b.finish(root));
    lowered.assert_clean();
    let mir = lowered.value.expect("a plan");
    let kept = mir
        .scalars
        .values()
        .any(|s| matches!(&s.kind, ScalarKind::Opaque { children, .. } if children.len() == 1));
    assert!(kept);
}

#[test]
fn a_star_over_an_open_schema_keeps_inputs_in_an_opaque_output() {
    let mut b = builder();
    let t = b.base_model(SPAN, "t", None, &[]);
    open(&mut b, t);
    let a = open_col(&mut b, "a", t);
    let one = b.number(SPAN, "1");
    let cond = b.binary(SPAN, BinaryOp::Eq, a, one);
    let root = select(&mut b, vec![], vec![star()], vec![t], |s| {
        s.filter = Some(cond)
    });
    let lowered = lower_to_mir(&b.finish(root));
    lowered.assert_clean();
    let mir = lowered.value.expect("a plan");
    let RelOp::Opaque { inputs, open, .. } = &mir.rels[mir.root].op else {
        panic!("{}", print_mir(&mir));
    };
    assert!(*open);
    assert!(matches!(mir.rels[inputs[0]].op, RelOp::Filter { .. }));
    assert!(lowered
        .degraded
        .iter()
        .any(|d| d.kind == DegradationKind::OpenSchema));
}

#[test]
fn open_scans_carry_only_the_columns_the_query_reads() {
    let mut b = builder();
    let t = b.base_model(SPAN, "t", None, &[]);
    open(&mut b, t);
    let a = open_col(&mut b, "a", t);
    let items = vec![item(a, "", &mut b)];
    let root = select(&mut b, vec![], items, vec![t], |_| {});
    let mir = clean(&b.finish(root));
    let printed = print_mir(&mir);
    assert!(
        printed.contains("[a#0:") && printed.contains(" open"),
        "{printed}"
    );
}

#[test]
fn the_cte_over_open_star_pattern_lowers_with_demanded_columns() {
    // with src as (select * from ref), final as (select a from src) select * from final
    let mut b = builder();
    let base = b.base_model(SPAN, "raw", None, &[]);
    open(&mut b, base);
    let src_q = select(&mut b, vec![], vec![star()], vec![base], |_| {});
    let src = b.cte(SPAN, "src", src_q);
    let from_src = b.base_cte(SPAN, src, None, &[]);
    open(&mut b, from_src);
    let a = open_col(&mut b, "a", from_src);
    let items = vec![item(a, "", &mut b)];
    let final_q = select(&mut b, vec![], items, vec![from_src], |_| {});
    let fin = b.cte(SPAN, "final", final_q);
    let from_final = b.base_cte(SPAN, fin, None, &["a"]);
    let root = select(
        &mut b,
        vec![src, fin],
        vec![star()],
        vec![from_final],
        |_| {},
    );
    let lowered = lower_to_mir(&b.finish(root));
    lowered.assert_clean();
    let mir = lowered.value.expect("a plan");
    assert_eq!(mir.root_columns().len(), 1);
    insta::assert_snapshot!(print_mir(&mir));
}

#[test]
fn an_ambiguous_open_reference_is_opaque_rather_than_guessed() {
    let mut b = builder();
    let (x, y) = (
        b.base_model(SPAN, "x", None, &[]),
        b.base_model(SPAN, "y", None, &[]),
    );
    open(&mut b, x);
    open(&mut b, y);
    let col = b.col_expr(SPAN, "a", Some(Resolution::Candidates(vec![x, y])));
    let items = vec![item(col, "", &mut b)];
    let root = select(&mut b, vec![], items, vec![x, y], |_| {});
    let lowered = lower_to_mir(&b.finish(root));
    lowered.assert_clean();
    assert_eq!(lowered.fidelity, Fidelity::Partial);
}

#[test]
fn a_recursive_cte_is_opaque_and_keeps_its_anchor() {
    let mut b = builder();
    let base = b.base_model(SPAN, "seed", None, &["a"]);
    let anchor = select(&mut b, vec![], vec![star()], vec![base], |_| {});
    let t2 = b.base_model(SPAN, "t2", None, &["a"]);
    let term = select(&mut b, vec![], vec![star()], vec![t2], |_| {});
    let body = b.query(
        SPAN,
        vec![],
        Body::SetOp {
            op: SetOpKind::Union,
            all: true,
            by_name: false,
            l: anchor,
            r: term,
        },
    );
    let cte = b.cte(SPAN, "r", body);
    b.hir.ctes[cte].recursive = true;
    let from = b.base_cte(SPAN, cte, None, &["a"]);
    let root = select(&mut b, vec![cte], vec![star()], vec![from], |_| {});
    let lowered = lower_to_mir(&b.finish(root));
    lowered.assert_clean();
    assert!(lowered
        .degraded
        .iter()
        .any(|d| d.kind == DegradationKind::RecursiveCte));
    let mir = lowered.value.expect("a plan");
    let body = mir.ctes.values().next().expect("cte").body;
    assert!(matches!(&mir.rels[body].op, RelOp::Opaque { inputs, .. } if inputs.len() == 1));
    insta::assert_snapshot!(print_mir(&mir));
}

#[test]
fn set_operations_join_arm_types_and_stay_set_ops() {
    let mut b = builder();
    let l = typed_from(&mut b, "l", &[("a", int_type())]);
    let dec = ColType::new(
        SqlType::Decimal {
            p: Some(10),
            s: Some(2),
        },
        Nullability::Nullable,
    );
    let r = typed_from(&mut b, "r", &[("a", dec)]);
    let (ql, qr) = (
        select(&mut b, vec![], vec![star()], vec![l], |_| {}),
        select(&mut b, vec![], vec![star()], vec![r], |_| {}),
    );
    let root = b.query(
        SPAN,
        vec![],
        Body::SetOp {
            op: SetOpKind::Intersect,
            all: false,
            by_name: false,
            l: ql,
            r: qr,
        },
    );
    let mir = clean(&b.finish(root));
    let out = mir.root_columns()[0];
    assert!(matches!(mir.cols[out].ty.ty, SqlType::Decimal { .. }));
    assert_eq!(mir.cols[out].ty.null, Nullability::Nullable);
    assert!(matches!(
        mir.rels[mir.root].op,
        RelOp::SetOp {
            op: SetOpKind::Intersect,
            ..
        }
    ));
}

#[test]
fn a_long_union_chain_flattens_without_consuming_depth() {
    let mut b = builder();
    let t = b.base_model(SPAN, "t", None, &["a"]);
    let mut acc = select(&mut b, vec![], vec![star()], vec![t], |_| {});
    for i in 0..200 {
        let next = b.base_model(SPAN, &format!("t{i}"), None, &["a"]);
        let arm = select(&mut b, vec![], vec![star()], vec![next], |_| {});
        acc = b.query(
            SPAN,
            vec![],
            Body::SetOp {
                op: SetOpKind::Union,
                all: true,
                by_name: false,
                l: acc,
                r: arm,
            },
        );
    }
    let mir = clean(&b.finish(acc));
    assert!(matches!(&mir.rels[mir.root].op, RelOp::SetOp { inputs, .. } if inputs.len() == 201));
}

#[test]
fn values_lowers_to_a_union_of_projects() {
    let mut b = builder();
    let (one, two) = (b.number(SPAN, "1"), b.number(SPAN, "2.5"));
    let root = b.query(SPAN, vec![], Body::Values(vec![vec![one], vec![two]]));
    let lowered = lower_to_mir(&b.finish(root));
    lowered.assert_clean();
    assert_eq!(lowered.fidelity, Fidelity::Complete);
    let mir = lowered.value.expect("a plan");
    assert!(matches!(
        mir.cols[mir.root_columns()[0]].ty.ty,
        SqlType::Decimal { .. }
    ));
}

#[test]
fn a_cte_used_before_its_definition_is_opaque() {
    let mut b = builder();
    let t = b.base_model(SPAN, "t", None, &["a"]);
    let inner = select(&mut b, vec![], vec![star()], vec![t], |_| {});
    let cte = b.cte(SPAN, "c", inner);
    let from = b.base_cte(SPAN, cte, None, &["a"]);
    let root = select(&mut b, vec![], vec![star()], vec![from], |_| {});
    let lowered = lower_to_mir(&b.finish(root));
    lowered.assert_clean();
    assert_eq!(lowered.fidelity, Fidelity::Partial);
}

#[test]
fn query_nesting_past_the_cap_degrades_instead_of_overflowing() {
    let mut b = builder();
    let t = b.base_model(SPAN, "t", None, &["a"]);
    let mut q = select(&mut b, vec![], vec![star()], vec![t], |_| {});
    for _ in 0..300 {
        let from = add_from(
            &mut b,
            FromKind::Derived {
                q,
                lateral: false,
                alias: None,
            },
            &["a"],
        );
        q = select(&mut b, vec![], vec![star()], vec![from], |_| {});
    }
    let lowered = lower_to_mir(&b.finish(q));
    lowered.assert_clean();
    assert!(lowered
        .degraded
        .iter()
        .any(|d| d.kind == DegradationKind::CapExceeded));
}

#[test]
fn a_missing_root_declines() {
    let hir = builder().hir;
    let lowered = lower_to_mir(&hir);
    assert_eq!(lowered.fidelity, Fidelity::Declined);
    assert!(lowered.value.is_none());
}

#[test]
fn out_of_range_ids_drop_the_level_without_panicking() {
    let mut b = builder();
    let bad = b.col_expr(
        SPAN,
        "a",
        Some(Resolution::Bound {
            from: Idx::from_raw(99),
            col: 0,
        }),
    );
    let missing = cogitator_ir::hir::ExprId::from_raw(4096);
    let items = vec![item(bad, "", &mut b), item(missing, "m", &mut b)];
    let root = select(&mut b, vec![], items, vec![Idx::from_raw(77)], |_| {});
    let lowered = lower_to_mir(&b.finish(root));
    assert!(lowered.value.is_none());
    assert_eq!(lowered.fidelity, Fidelity::Declined);
    assert!(lowered
        .violations
        .iter()
        .any(|v| v.invariant == Invariant::HirIdRange));
}

#[test]
fn a_query_id_cycle_stops_at_the_depth_cap() {
    let mut b = builder();
    let from = add_from(
        &mut b,
        FromKind::Derived {
            q: Idx::from_raw(0),
            lateral: false,
            alias: None,
        },
        &[],
    );
    let root = select(&mut b, vec![], vec![star()], vec![from], |_| {});
    assert_eq!(root.raw(), 0, "the cycle above relies on this id");
    let lowered = lower_to_mir(&b.finish(root));
    assert!(lowered
        .degraded
        .iter()
        .any(|d| d.kind == DegradationKind::CapExceeded));
}

#[test]
fn lowering_is_deterministic() {
    let build = || {
        let mut b = builder();
        let t = b.base_model(SPAN, "t", None, &["a", "b"]);
        let (k, v) = (b.column(SPAN, "a", t, 0), b.column(SPAN, "b", t, 1));
        let key = b.column(SPAN, "a", t, 0);
        let total = call(&mut b, "sum", vec![v]);
        let items = vec![item(k, "", &mut b), item(total, "s", &mut b)];
        let root = select(&mut b, vec![], items, vec![t], |s| {
            s.group_by = GroupBy::Exprs(vec![key]);
        });
        print_mir(&clean(&b.finish(root)))
    };
    assert_eq!(build(), build());
}

#[test]
fn a_cte_target_outside_the_arena_is_a_violation_not_a_panic() {
    let mut b = builder();
    let from = add_from(
        &mut b,
        FromKind::Base {
            target: RelTarget::Cte(Idx::from_raw(5)),
            alias: None,
        },
        &["a"],
    );
    let root = select(
        &mut b,
        vec![Idx::from_raw(5)],
        vec![star()],
        vec![from],
        |_| {},
    );
    let lowered = lower_to_mir(&b.finish(root));
    assert!(lowered.value.is_none());
}
