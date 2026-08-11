//! Tests for the [`Language`] metadata that the rest of the tool is driven by.
//!
//! None of this parses anything. It exists because adding a language means touching four places
//! that have no compiler-enforced relationship to each other -- the variant, [`Language::ALL`],
//! [`Language::from_extension`], and the extension and name a variant reports -- and leaving any
//! one of them out is silent. A file whose extension isn't mapped is skipped by the directory walk
//! exactly as if it held no statements, and a variant missing from `ALL` is invisible to the
//! `FromStr` impl and to the "expected one of" list in its error.
//!
//! So each test below walks `ALL` rather than naming variants, and the one thing an omission
//! cannot hide from is a test that enumerates.

use std::collections::HashSet;
use std::str::FromStr;
use yadr::Language;

/// The directory walk reaches a language only through the extension of the file it is looking at,
/// so a variant whose own extension doesn't map back to it can never be selected.
#[test]
fn every_language_is_reachable_from_its_own_extension() {
    for language in Language::ALL {
        assert_eq!(
            Language::from_extension(language.extension()),
            Some(*language),
            "{language} reports the extension {:?}, which maps to something else",
            language.extension(),
        );
    }
}

#[test]
fn every_language_is_reachable_from_its_own_name() {
    for language in Language::ALL {
        assert_eq!(
            Language::from_str(language.name()).as_ref(),
            Ok(language),
            "{language} reports the name {:?}, which does not parse back",
            language.name(),
        );
    }
}

/// Two variants sharing an extension or a name would make one of them unreachable, and the
/// round-trip tests above would still pass for whichever one won.
#[test]
fn extensions_and_names_are_unique() {
    let mut extensions = HashSet::new();
    let mut names = HashSet::new();
    for language in Language::ALL {
        assert!(
            extensions.insert(language.extension()),
            "more than one language claims the extension {:?}",
            language.extension(),
        );
        assert!(
            names.insert(language.name()),
            "more than one language claims the name {:?}",
            language.name(),
        );
    }
}

/// `ALL` is what `FromStr` searches and what the unknown-language error lists, so a variant left
/// out of it is unusable from the command line however well it parses.
#[test]
fn all_lists_every_supported_language() {
    for language in [
        Language::Rust,
        Language::Python,
        Language::Nix,
        Language::TypeScript,
    ] {
        assert!(
            Language::ALL.contains(&language),
            "{language} is missing from Language::ALL",
        );
    }
}

#[test]
fn typescript_is_selected_for_ts_files() {
    assert_eq!(
        Language::from_extension("ts"),
        Some(Language::TypeScript),
        "a .ts file has to reach the TypeScript parser",
    );
}

/// Extensions are compared as they arrive from the path, without a leading `.` and without case
/// folding, so neither spelling should be mistaken for a supported language.
#[test]
fn unsupported_extensions_are_not_languages() {
    for extension in [".ts", "tsx", "mts", "cts", "js", "md", ""] {
        assert_eq!(
            Language::from_extension(extension),
            None,
            "{extension:?} is not an extension this version parses",
        );
    }
}
