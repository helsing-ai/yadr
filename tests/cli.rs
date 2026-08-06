//! End-to-end tests for the `yadr` binary.
//!
//! Everything else in the test suite exercises the parser directly. These tests instead run the
//! real executable over the fixture trees in `tests/fixtures/`, so they cover the parts that only
//! exist in `main.rs`: argument parsing, the directory walk, the exclude rules, the shape of each
//! subcommand's output, and the exit status.
//!
//! The fixtures are deliberately kept small and boring. They exist to be parsed, not read.

use assert_cmd::Command;
use predicates::prelude::*;

/// A tree of three files, one per supported language, holding four valid Y-Statements between
/// them. `storage.rs` holds two so that per-file grouping in `list` output gets exercised.
const CLEAN: &str = "tests/fixtures/clean";

/// A tree holding a single Y-Statement whose second paragraph doesn't follow the format.
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
        .stdout(predicate::str::contains(
            "2024-01-15 Store timestamps as UTC",
        ))
        .stdout(predicate::str::contains(
            "2024-01-22 Fail closed on an unreadable file",
        ))
        .stdout(predicate::str::contains(
            "2024-03-02 Retry with exponential backoff",
        ))
        .stdout(predicate::str::contains(
            "2024-04-09 Pin the toolchain in the lock file",
        ));
}

/// The two statements in `storage.rs` should appear under a single `==>` heading for that file,
/// not one heading each.
#[test]
fn list_groups_statements_under_their_file() {
    let stdout = stdout_of(yadr().args(["ls", CLEAN]).assert().success());

    let headings: Vec<_> = stdout.lines().filter(|l| l.starts_with("==>")).collect();
    assert_eq!(headings.len(), 3, "one heading per file, in:\n{stdout}");
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
            "2024-01-15 Store timestamps as UTC (last changed: 2024-02-01)",
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
        stdout.contains("tests/fixtures/clean/retry.py:3"),
        "no source location in:\n{stdout}"
    );
    // reflowed onto one line per paragraph, rather than as it was wrapped in the comment
    assert!(
        stdout.contains(
            "In the context of talking to an upstream service that occasionally rejects requests, \
             we faced the question of how quickly to retry."
        ),
        "paragraph was not reflowed in:\n{stdout}"
    );
    // and only the statement that was asked for
    assert!(
        !stdout.contains("2024-01-15"),
        "showed an unrequested statement in:\n{stdout}"
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
