//! The profiles.yml auth manager: Snowflake auth classification, per-method
//! validation, read scope, secret redaction, and the profiles.yml
//! diagnostics arm.

use std::path::{Path, PathBuf};

use cogitator_analysis::profiles::{inventory, AuthMethod, ProfileScope};
use cogitator_analysis::util::get_dialect;
use cogitator_analysis::yaml_features::yaml_diagnostics;
use pretty_assertions::assert_eq;

const PATH: &str = "/tmp/profiles.yml";

fn target_auth(yaml: &str) -> (AuthMethod, Vec<String>) {
    let inv = inventory(Path::new(PATH), yaml, &ProfileScope::All);
    let target = &inv.profiles[0].targets[0];
    (
        target.auth.method,
        target
            .auth
            .issues
            .iter()
            .map(|i| i.message.clone())
            .collect(),
    )
}

fn snowflake_target(body: &str) -> String {
    format!("p:\n  target: dev\n  outputs:\n    dev:\n      type: snowflake\n{body}")
}

#[test]
fn password_auth_classified_and_validated() {
    let (method, issues) = target_auth(&snowflake_target(
        "      account: acc\n      user: u\n      password: secret-value\n",
    ));
    assert_eq!(method, AuthMethod::Password);
    assert_eq!(issues, Vec::<String>::new());

    let (method, issues) = target_auth(&snowflake_target("      account: acc\n      user: u\n"));
    assert_eq!(method, AuthMethod::Unspecified);
    assert!(
        issues.iter().any(|i| i.contains("no auth method")),
        "{issues:?}"
    );
}

#[test]
fn key_pair_auth_checks_the_key_file() {
    let missing = snowflake_target(
        "      account: acc\n      user: u\n      private_key_path: /nope/rsa_key.p8\n",
    );
    let (method, issues) = target_auth(&missing);
    assert_eq!(method, AuthMethod::KeyPair);
    assert!(issues.iter().any(|i| i.contains("not found")), "{issues:?}");

    let dir = std::env::temp_dir().join(format!("cogitator-auth-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let key = dir.join("rsa_key.p8");
    std::fs::write(&key, "-----BEGIN PRIVATE KEY-----").unwrap();
    let ok = snowflake_target(&format!(
        "      account: acc\n      user: u\n      private_key_path: {}\n",
        key.display()
    ));
    let (method, issues) = target_auth(&ok);
    assert_eq!(method, AuthMethod::KeyPair);
    assert_eq!(issues, Vec::<String>::new());
}

#[test]
fn authenticator_variants() {
    let (method, issues) = target_auth(&snowflake_target(
        "      account: acc\n      user: u\n      authenticator: externalbrowser\n",
    ));
    assert_eq!(method, AuthMethod::ExternalBrowser);
    assert_eq!(issues, Vec::<String>::new());

    let (method, issues) = target_auth(&snowflake_target(
        "      account: acc\n      user: u\n      authenticator: oauth\n",
    ));
    assert_eq!(method, AuthMethod::Oauth);
    assert!(issues.iter().any(|i| i.contains("token")), "{issues:?}");

    let (method, _) = target_auth(&snowflake_target(
        "      account: acc\n      user: u\n      authenticator: username_password_mfa\n      password: x\n",
    ));
    assert_eq!(method, AuthMethod::UsernamePasswordMfa);

    let (method, issues) = target_auth(&snowflake_target(
        "      account: acc\n      user: u\n      authenticator: sspi\n",
    ));
    assert_eq!(method, AuthMethod::Unspecified);
    assert!(
        issues.iter().any(|i| i.contains("unknown authenticator")),
        "{issues:?}"
    );
}

#[test]
fn unresolved_env_vars_are_reported_with_their_name() {
    let yaml = snowflake_target(
        "      account: acc\n      user: u\n      password: \"{{ env_var('COGITATOR_TEST_UNSET_12345') }}\"\n",
    );
    let (method, issues) = target_auth(&yaml);
    assert_eq!(method, AuthMethod::Password);
    assert!(
        issues
            .iter()
            .any(|i| i.contains("COGITATOR_TEST_UNSET_12345")),
        "{issues:?}"
    );

    // a default silences the issue
    let yaml = snowflake_target(
        "      account: acc\n      user: u\n      password: \"{{ env_var('COGITATOR_TEST_UNSET_12345', 'fallback') }}\"\n",
    );
    let (_, issues) = target_auth(&yaml);
    assert_eq!(issues, Vec::<String>::new());
}

#[test]
fn scope_hides_profiles_outside_the_allowlist() {
    let yaml = "\
a:\n  target: dev\n  outputs:\n    dev:\n      type: duckdb\n\
b:\n  target: dev\n  outputs:\n    dev:\n      type: duckdb\n\
c:\n  target: dev\n  outputs:\n    dev:\n      type: duckdb\n";
    let allow = vec!["b".to_string()];
    let inv = inventory(
        Path::new(PATH),
        yaml,
        &ProfileScope::Allowed {
            project: "a",
            extra: &allow,
        },
    );
    let by_name = |n: &str| inv.profile(n).unwrap();
    assert!(by_name("a").allowed && !by_name("a").targets.is_empty());
    assert!(by_name("b").allowed);
    assert!(!by_name("c").allowed);
    assert!(
        by_name("c").targets.is_empty(),
        "scoped-out profile must stay unparsed"
    );
}

#[test]
fn report_json_never_contains_secret_values() {
    let yaml = snowflake_target(
        "      account: acc\n      user: u\n      password: hunter2-super-secret\n      token: tok-abc123\n",
    );
    let inv = inventory(Path::new(PATH), &yaml, &ProfileScope::All);
    let rendered = inv.to_json().to_string();
    assert!(!rendered.contains("hunter2-super-secret"), "{rendered}");
    assert!(!rendered.contains("tok-abc123"), "{rendered}");
    assert!(rendered.contains("\"method\":\"password\""), "{rendered}");
}

#[test]
fn profiles_yml_diagnostics_surface_auth_issues() {
    let yaml = snowflake_target("      user: u\n"); // no account, no auth
    let diagnostics = yaml_diagnostics("file:///tmp/profiles.yml", &yaml);
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("no auth method")),
        "{diagnostics:?}"
    );
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("`account` is missing")),
        "{diagnostics:?}"
    );
    // positions point into the file, not all at 0:0
    assert!(
        diagnostics.iter().any(|d| d.range.start.line > 0),
        "{diagnostics:?}"
    );
}

#[test]
fn target_override_switches_the_dialect() {
    let dir = std::env::temp_dir().join(format!("cogitator-target-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("profiles.yml"),
        "jaffle_shop:\n  target: dev\n  outputs:\n    dev:\n      type: duckdb\n    prod:\n      type: snowflake\n",
    )
    .unwrap();
    let dir = dir.to_str().unwrap();
    assert_eq!(get_dialect("jaffle_shop", "", dir, None).as_str(), "duckdb");
    assert_eq!(
        get_dialect("jaffle_shop", "", dir, Some("prod")).as_str(),
        "snowflake"
    );
    assert_eq!(
        get_dialect("jaffle_shop", "", dir, Some("nope")).as_str(),
        ""
    );
    let _ = PathBuf::from(dir);
}
