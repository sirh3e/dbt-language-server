//! `cogitator lint` and `cogitator format` as CI runs them: the binary on
//! a temp copy of the fixture, results on stdout, exit codes 0/1/2.

mod harness;
use harness::{jaffle, TestProject};

fn cogitator(project: &TestProject, args: &[&str]) -> std::process::Output {
    let home = project.home.to_string_lossy().into_owned();
    std::process::Command::new(env!("CARGO_BIN_EXE_cogitator"))
        .args(args)
        .current_dir(&project.root)
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env(
            "DBT_PROFILES_DIR",
            project.root.join(".dbt").to_string_lossy().into_owned(),
        )
        .output()
        .expect("the binary runs")
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn lint_is_clean_then_finds_a_broken_file() {
    let project = jaffle();
    let clean = cogitator(&project, &["lint"]);
    assert_eq!(clean.status.code(), Some(0), "{}", stdout(&clean));
    assert!(!stdout(&clean).contains("error"), "{}", stdout(&clean));

    let broken = project.root.join("models/broken.sql");
    std::fs::write(&broken, "select a,, b from {{ ref('customers') }}\n").unwrap();
    let found = cogitator(&project, &["lint"]);
    assert_eq!(found.status.code(), Some(1));
    let out = stdout(&found);
    assert!(
        out.contains("models/broken.sql:1:10: error sql-syntax: SQL syntax error"),
        "{out}"
    );
    assert!(out.contains("    Solution: "), "{out}");

    // only the named paths
    let one = cogitator(&project, &["lint", "models/customers.sql"]);
    assert_eq!(one.status.code(), Some(0), "{}", stdout(&one));

    // JSON and GitHub annotations
    let json = cogitator(&project, &["lint", "--format", "json"]);
    let items: serde_json::Value = serde_json::from_slice(&json.stdout).expect("json");
    let item = items
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["code"] == "sql-syntax")
        .expect("the finding");
    assert_eq!(
        (item["line"].as_u64(), item["column"].as_u64()),
        (Some(1), Some(10))
    );
    assert_eq!(item["severity"], "error");
    let github = stdout(&cogitator(&project, &["lint", "--format", "github"]));
    assert!(
        github.contains("::error file=models/broken.sql,line=1,col=10,"),
        "{github}"
    );
    assert!(github.contains("title=sql-syntax::"), "{github}");

    // cogitator.yml turns the check off
    std::fs::write(
        project.root.join("cogitator.yml"),
        "lint:\n  syntax: false\n",
    )
    .unwrap();
    let off = cogitator(&project, &["lint"]);
    assert_eq!(off.status.code(), Some(0), "{}", stdout(&off));
}

#[test]
fn lint_max_warnings_and_usage_errors() {
    let project = jaffle();
    // the fixture carries one dbt-yaml warning
    let strict = cogitator(&project, &["lint", "--max-warnings", "0"]);
    assert_eq!(strict.status.code(), Some(1), "{}", stdout(&strict));
    let missing = cogitator(&project, &["lint", "--project-dir", "no/such/dir"]);
    assert_eq!(missing.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&missing.stderr);
    assert!(stderr.contains("Solution:"), "{stderr}");
    assert!(missing.stdout.is_empty());
}

#[test]
fn format_check_then_format_then_clean() {
    let project = jaffle();
    let messy = project.root.join("models/messy.sql");
    std::fs::write(&messy, "select a,b from {{ ref('customers') }} where a>1\n").unwrap();
    let declined = project.root.join("models/declined.sql");
    let declined_text =
        "select a {% if var('x', false) %}, b{% endif %} from {{ ref('customers') }}\n";
    std::fs::write(&declined, declined_text).unwrap();

    let check = cogitator(&project, &["format", "--check", "--diff"]);
    assert_eq!(check.status.code(), Some(1));
    let out = stdout(&check);
    assert!(out.contains("would reformat models/messy.sql"), "{out}");
    assert!(out.contains("+++ b/models/messy.sql"), "{out}");
    let stderr = String::from_utf8_lossy(&check.stderr);
    assert!(stderr.contains("skipped models/declined.sql"), "{stderr}");
    assert_eq!(
        std::fs::read_to_string(&messy).unwrap(),
        "select a,b from {{ ref('customers') }} where a>1\n",
        "--check changes nothing"
    );

    let format = cogitator(&project, &["format"]);
    assert_eq!(format.status.code(), Some(0), "{}", stdout(&format));
    assert!(stdout(&format).contains("reformatted models/messy.sql"));

    let again = cogitator(&project, &["format", "--check"]);
    assert_eq!(again.status.code(), Some(0), "{}", stdout(&again));
    assert_eq!(
        std::fs::read_to_string(&declined).unwrap(),
        declined_text,
        "a declined file stays byte-identical"
    );
}
