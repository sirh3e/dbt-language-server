use super::*;

fn facts(text: &str) -> MacroBody {
    let tokens: Vec<_> = tokenize(text, false, SyntaxConfig::default())
        .collect::<Result<_, _>>()
        .unwrap();
    // skip the header: start after its first BlockEnd
    let from = tokens
        .iter()
        .position(|(t, _)| matches!(t, JToken::BlockEnd))
        .unwrap()
        + 1;
    body_facts(&tokens, from)
}

fn dispatch(name: &str, ns: Option<&str>) -> Option<DispatchCall> {
    Some(DispatchCall {
        name: name.into(),
        namespace: ns.map(Into::into),
    })
}

#[test]
fn body_facts_case_table() {
    let cases: &[(&str, bool, bool, Option<DispatchCall>)] = &[
        ("{% macro m(a) %}{{ a }}{% endmacro %}", false, false, None),
        (
            "{% macro m() %}{{ varargs[0] }}{% endmacro %}",
            true,
            false,
            None,
        ),
        (
            "{% macro m() %}{{ kwargs.x }}{% endmacro %}",
            false,
            true,
            None,
        ),
        (
            "{% macro m(a) %}{{ return(adapter.dispatch('m', 'pkg')(a)) }}{% endmacro %}",
            false,
            false,
            dispatch("m", Some("pkg")),
        ),
        (
            "{% macro m() %}{{ adapter.dispatch('m')() }}{% endmacro %}",
            false,
            false,
            dispatch("m", None),
        ),
        (
            "{% macro m() %}{{ adapter.dispatch('m', macro_namespace='p')() }}{% endmacro %}",
            false,
            false,
            dispatch("m", Some("p")),
        ),
        // a later macro's body is not this one's
        (
            "{% macro m() %}x{% endmacro %}{% macro n() %}{{ varargs }}{% endmacro %}",
            false,
            false,
            None,
        ),
        // a dispatch with a computed name is no literal dispatch
        (
            "{% macro m() %}{{ adapter.dispatch(name)() }}{% endmacro %}",
            false,
            false,
            None,
        ),
    ];
    for (text, varargs, kwargs, dispatch) in cases {
        let got = facts(text);
        assert_eq!(got.reads_varargs, *varargs, "{text}");
        assert_eq!(got.reads_kwargs, *kwargs, "{text}");
        assert_eq!(&got.dispatch, dispatch, "{text}");
    }
}

#[test]
fn dispatch_sites_carry_offsets() {
    let text = "{{ adapter.dispatch('x', 'p')() }} {{ adapter.dispatch('y')() }}";
    let sites = dispatch_sites(text);
    assert_eq!(sites.len(), 2);
    assert_eq!(
        &text[sites[0].start as usize..sites[0].end as usize],
        "adapter.dispatch('x', 'p')"
    );
    assert_eq!(sites[1].call, dispatch("y", None).unwrap());
}

#[test]
fn call_shape_case_table() {
    let kw = |pairs: &[(&str, u32)]| pairs.iter().map(|(n, o)| (n.to_string(), *o)).collect();
    let cases: &[(&str, Option<CallShape>)] = &[
        ("m()", Some(CallShape::default())),
        (
            "m(1, f(2, 3), [4, 5])",
            Some(CallShape {
                positional: 3,
                ..Default::default()
            }),
        ),
        (
            "pkg.m(1, b=2, c='x')",
            Some(CallShape {
                positional: 1,
                kwargs: kw(&[("b", 9), ("c", 14)]),
                splat: false,
            }),
        ),
        (
            "m(a=1, a=2)",
            Some(CallShape {
                positional: 0,
                kwargs: kw(&[("a", 2), ("a", 7)]),
                splat: false,
            }),
        ),
        (
            "m(*xs)",
            Some(CallShape {
                splat: true,
                ..Default::default()
            }),
        ),
        (
            "m(**kw)",
            Some(CallShape {
                splat: true,
                ..Default::default()
            }),
        ),
        ("m(1,", None),
        ("m", None),
        ("m(1) + 2", None),
    ];
    for (source, want) in cases {
        assert_eq!(&call_shape(source), want, "{source}");
    }
}
