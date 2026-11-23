use super::*;

fn params(spec: &[(&str, Option<&str>)]) -> Vec<MacroParam> {
    spec.iter()
        .map(|(n, d)| MacroParam {
            name: n.to_string(),
            default: d.map(str::to_string),
        })
        .collect()
}

fn shape(positional: usize, kwargs: &[&str]) -> CallShape {
    CallShape {
        positional,
        kwargs: kwargs
            .iter()
            .enumerate()
            .map(|(i, k)| (k.to_string(), i as u32 * 10))
            .collect(),
        splat: false,
    }
}

fn body(varargs: bool, kwargs: bool) -> MacroBody {
    MacroBody {
        reads_varargs: varargs,
        reads_kwargs: kwargs,
        dispatch: None,
    }
}

/// (label, params, body, call, problems)
type Case<'a> = (
    &'a str,
    &'a [MacroParam],
    MacroBody,
    CallShape,
    Vec<Problem>,
);

#[test]
fn judge_case_table() {
    let ab = params(&[("a", None), ("b", Some("1"))]);
    let cases: Vec<Case<'_>> = vec![
        ("exact", &ab, body(false, false), shape(2, &[]), vec![]),
        (
            "default omitted",
            &ab,
            body(false, false),
            shape(1, &[]),
            vec![],
        ),
        (
            "required by kwarg",
            &ab,
            body(false, false),
            shape(0, &["a"]),
            vec![],
        ),
        (
            "too many",
            &ab,
            body(false, false),
            shape(3, &[]),
            vec![Problem::TooMany { given: 3, takes: 2 }],
        ),
        (
            "varargs takes extras",
            &ab,
            body(true, false),
            shape(5, &[]),
            vec![],
        ),
        (
            "missing required",
            &ab,
            body(false, false),
            shape(0, &[]),
            vec![Problem::Missing {
                names: vec!["a".into()],
            }],
        ),
        (
            "unknown kwarg with did-you-mean",
            &ab,
            body(false, false),
            shape(1, &["bb"]),
            vec![Problem::UnknownKwarg {
                name: "bb".into(),
                offset: 0,
                suggestions: vec!["b".into()],
            }],
        ),
        (
            "kwargs takes unknown",
            &ab,
            body(false, true),
            shape(1, &["zz"]),
            vec![],
        ),
        (
            "duplicate kwarg",
            &ab,
            body(false, false),
            shape(1, &["b", "b"]),
            vec![Problem::DuplicateKwarg {
                name: "b".into(),
                offset: 10,
            }],
        ),
        (
            "kwarg also given positionally",
            &ab,
            body(false, false),
            shape(1, &["a"]),
            vec![Problem::DuplicateKwarg {
                name: "a".into(),
                offset: 0,
            }],
        ),
        (
            "splat is unknowable",
            &ab,
            body(false, false),
            CallShape {
                splat: true,
                ..shape(9, &["q"])
            },
            vec![],
        ),
    ];
    for (label, params, body, shape, want) in cases {
        assert_eq!(judge(params, &body, &shape), want, "{label}");
    }
}
