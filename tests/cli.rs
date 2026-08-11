//! End-to-end tests for the `yadr` binary.
//!
//! Everything else in the test suite exercises the parser directly. These tests instead run the
//! real executable over the fixture trees in `tests/fixtures/`, so they cover the parts that only
//! exist in `main.rs`: argument parsing, the directory walk, the exclude rules, the shape of each
//! subcommand's output, and the exit status.
//!
//! The fixtures are deliberately kept small and boring. They exist to be parsed, not read, so a
//! statement is identified here by its date rather than by its title.

use assert_cmd::Command;
use predicates::prelude::*;

/// A tree of files, one per supported language, holding valid Y-Statements.
/// `storage.rs` holds two so that per-file grouping in `list` output gets exercised.
const CLEAN: &str = "tests/fixtures/clean";

/// A tree holding a single Y-Statement, dated 2024-05-20, whose second paragraph doesn't follow
/// the format.
const BROKEN: &str = "tests/fixtures/broken";

fn yadr() -> Command {
    Command::cargo_bin("yadr").expect("the binary is built by the same `cargo test` invocation")
}

/// Captures stdout with path separators normalised to `/`.
///
/// `yadr` prints whatever the platform's path type renders, which on Windows means backslashes.
/// The tests care about *which* files were named, not about how the host spells a path, so they
/// compare against forward slashes throughout and normalise here.
fn stdout_of(assert: assert_cmd::assert::Assert) -> String {
    let raw = String::from_utf8(assert.get_output().stdout.clone()).expect("output is utf-8");
    raw.replace('\\', "/")
}

#[test]
fn list_finds_statements_in_every_supported_language() {
    yadr()
        .args(["ls", CLEAN])
        .assert()
        .success()
        .stdout(predicate::str::contains("2024-01-15 <title>"))
        .stdout(predicate::str::contains("2024-01-22 <title>"))
        .stdout(predicate::str::contains("2024-03-02 <title>"))
        .stdout(predicate::str::contains("2024-04-09 <title>"))
        .stdout(predicate::str::contains("2024-06-11 Send the auth token in a header"))
        .stdout(predicate::str::contains(
            "2024-07-23 Key the cache by the full request URL",
        ))
        .stdout(predicate::str::contains(
            "2024-08-14 Take the panel's contents as children",
        ))
        .stdout(predicate::str::contains(
            "2024-09-05 Render the banner as a sibling of the page content",
        ));
}

/// The two statements in `storage.rs` should appear under a single `==>` heading for that file,
/// not one heading each.
#[test]
fn list_groups_statements_under_their_file() {
    let stdout = stdout_of(yadr().args(["ls", CLEAN]).assert().success());

    let headings: Vec<_> = stdout.lines().filter(|l| l.starts_with("==>")).collect();
    assert_eq!(headings.len(), 7, "one heading per file, in:\n{stdout}");
    assert_eq!(
        stdout.matches("storage.rs").count(),
        1,
        "storage.rs holds two statements but should be named once, in:\n{stdout}"
    );
}

/// A statement with a change log should advertise the date of its most recent change, so that
/// `yadr ls` is usable for spotting decisions that have been revisited.
#[test]
fn list_reports_the_most_recent_change() {
    yadr()
        .args(["ls", CLEAN])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "2024-01-15 <title> (last changed: 2024-02-01)",
        ));
}

#[test]
fn list_only_files_names_files_and_nothing_else() {
    let stdout = stdout_of(yadr().args(["ls", "-L", CLEAN]).assert().success());

    let mut lines: Vec<_> = stdout.lines().collect();
    lines.sort_unstable();
    assert_eq!(
        lines,
        [
            "tests/fixtures/clean/banner.jsx",
            "tests/fixtures/clean/cache.js",
            "tests/fixtures/clean/client.ts",
            "tests/fixtures/clean/panel.tsx",
            "tests/fixtures/clean/retry.py",
            "tests/fixtures/clean/storage.rs",
            "tests/fixtures/clean/toolchain.nix",
        ],
        "in:\n{stdout}"
    );
}

/// `show` takes a prefix of the title, which in practice is a date.
#[test]
fn show_prints_one_statement_in_full() {
    let stdout = stdout_of(
        yadr()
            .args(["show", "2024-03-02", CLEAN])
            .assert()
            .success(),
    );

    // the source location, so the reader can go and find it
    assert!(
        stdout.contains("tests/fixtures/clean/retry.py:6"),
        "no source location in:\n{stdout}"
    );
    // reflowed onto one line per paragraph, rather than as it was wrapped in the comment
    assert!(
        stdout.contains("In the context of <ctx>, we faced <con>."),
        "paragraph was not reflowed in:\n{stdout}"
    );
    // and only the statement that was asked for
    assert!(
        !stdout.contains("2024-01-15"),
        "showed an unrequested statement in:\n{stdout}"
    );
}

/// `banner.jsx` writes its statement inside JSX markup, as a run of one-line `{/* ... */}`
/// containers rather than as one block comment. Each container is a `jsx_expression` of its own,
/// so the comments are only-children rather than siblings and the walk has to group them by the
/// lines they occupy.
///
/// Asserting on the reflowed prose rather than on the title is what makes this a test of the
/// grouping: a title is found from the first container alone, whereas a paragraph split across two
/// containers only comes out whole if the run was joined. The last assertion covers the other
/// half, that grouping stops where the lines stop being consecutive.
#[test]
fn show_joins_a_statement_split_across_jsx_containers() {
    let stdout = stdout_of(
        yadr()
            .args(["show", "2024-09-05", CLEAN])
            .assert()
            .success(),
    );

    assert!(
        stdout.contains("tests/fixtures/clean/banner.jsx:16"),
        "no source location in:\n{stdout}"
    );
    // spread over two containers in the fixture, so this is one line only if they were joined
    assert!(
        stdout.contains(
            "In the context of a status banner that has to be announced as soon as it appears, \
             we faced the question of where to mount it in the tree."
        ),
        "paragraph split across containers was not joined in:\n{stdout}"
    );
    // a lone JSX comment sits below the statement, separated from it by a line of markup, and so
    // must not have been swept up as the statement's tail
    assert!(
        !stdout.contains("A lone JSX comment"),
        "an unrelated JSX comment joined the statement in:\n{stdout}"
    );
}

#[test]
fn check_is_silent_and_succeeds_on_a_well_formed_tree() {
    yadr()
        .args(["check", CLEAN])
        .assert()
        .success()
        .stdout(predicate::str::is_empty());
}

#[test]
fn check_fails_and_points_at_the_bad_paragraph() {
    yadr()
        .args(["check", BROKEN])
        .assert()
        .failure()
        .stderr(predicate::str::contains("malformed.rs"))
        .stderr(predicate::str::contains("in paragraph 2 of that statement"))
        .stderr(predicate::str::contains(
            r#"expected "We decided for <..>, and neglected <..>.""#,
        ));
}

#[test]
fn exclude_skips_matching_paths() {
    yadr()
        .args(["ls", "--exclude", "*.py", CLEAN])
        .assert()
        .success()
        .stdout(predicate::str::contains("2024-01-15"))
        .stdout(predicate::str::contains("2024-04-09"))
        .stdout(predicate::str::contains("2024-03-02").not());
}

/// Excluding the one malformed file should turn a failing `check` into a passing one; this is how
/// a project would carve out a directory of test data from its own CI check.
#[test]
fn exclude_applies_to_check() {
    yadr()
        .args(["check", "--exclude", "malformed.rs", BROKEN])
        .assert()
        .success();
}

/// The path is optional and defaults to the working directory. Point the child process at a
/// fixture tree so this doesn't depend on where the test happens to run from.
#[test]
fn root_defaults_to_the_working_directory() {
    yadr()
        .arg("ls")
        .current_dir(CLEAN)
        .assert()
        .success()
        .stdout(predicate::str::contains("2024-04-09"));
}

/// `yadr ls <path>` is the natural way to type this, and it used to be an error: the path had to
/// come *before* the subcommand. Guard against regressing back to that.
#[test]
fn path_is_accepted_after_the_subcommand() {
    yadr().args(["ls", CLEAN]).assert().success();
    yadr().args(["check", CLEAN]).assert().success();
    yadr()
        .args(["show", "2024-01-15", CLEAN])
        .assert()
        .success();
}
