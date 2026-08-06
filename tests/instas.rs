use yadr::*;

// this has to be a macro rather than a function because `insta` derives each snapshot's name from
// the enclosing function, so a shared helper function would give every test the same snapshot name.
macro_rules! harness {
    ($lang:ident, $source:literal) => {
        // avoid noisy (and change-prone) bits in output
        // NOTE: we don't .unwrap() the Result since that'd error when multiple tests do this
        let _ = miette::set_hook(Box::new(|_| {
            Box::new(
                miette::MietteHandlerOpts::new()
                    .width(1000)
                    .context_lines(3)
                    .tab_width(4)
                    .terminal_links(true)
                    .color(false)
                    .unicode(false)
                    .build(),
            )
        }));

        let mut yadrs = Vec::new();
        let res = find_all(
            $source,
            Language::$lang,
            |line: usize, mut yadr: YAdr<'_>| {
                yadr.tidy();
                yadrs.push((line, yadr.into_owned()));
                Ok(true)
            },
        );
        let res = res.map(|_| yadrs);
        match res {
            Ok(v) => insta::assert_debug_snapshot!(v),
            Err(e) => insta::assert_snapshot!(format!("{e:?}")),
        }
    };
}

#[test]
fn stars() {
    harness!(
        Rust,
        r#"
/*
 * YADR: 2023-11-29 foo-bar also
 * In the context of <use case/user story u>, we faced <concern c>.
 *
 * We decided for <option o>, and neglected <other options>.
 *
 * We did this to achieve <system qualities/desired consequences>, accepting
 * <downside d/undesired consequences>.
 *
 * We think this is the right trade-off because <additional rationale>.
 *
 * Also, foobar to the baz.
 */
"#
    );
}

#[test]
fn stripes() {
    harness!(
        Rust,
        r#"
// YADR: 2023-11-30 bar-baz
// In the context of <use case/user story u>, we faced <concern c>.
//
// We decided for <option o>, and neglected <other options>.
//
// We did this to achieve <system qualities/desired consequences>, accepting
// <downside d/undesired consequences>.
//
// We think this is the right trade-off because <additional rationale>.
"#
    );
}

#[test]
fn hashes_nix() {
    harness!(
        Nix,
        r#"
# YADR: 2023-11-30 bar-baz
# In the context of <use case/user story u>, we faced <concern c>.
#
# We decided for <option o>, and neglected <other options>.
#
# We did this to achieve <system qualities/desired consequences>, accepting
# <downside d/undesired consequences>.
#
# We think this is the right trade-off because <additional rationale>.
"#
    );
}

#[test]
fn block_nix() {
    harness!(
        Nix,
        r#"
/* YADR: 2023-11-30 bar-baz
 * In the context of <use case/user story u>, we faced <concern c>.
 *
 * We decided for <option o>, and neglected <other options>.
 *
 * We did this to achieve <system qualities/desired consequences>, accepting
 * <downside d/undesired consequences>.
 *
 * We think this is the right trade-off because <additional rationale>.
 */
"#
    );
}

#[test]
fn double_block_nix() {
    harness!(
        Nix,
        r#"
/** YADR: 2023-11-30 bar-baz
 * In the context of <use case/user story u>, we faced <concern c>.
 *
 * We decided for <option o>, and neglected <other options>.
 *
 * We did this to achieve <system qualities/desired consequences>, accepting
 * <downside d/undesired consequences>.
 *
 * We think this is the right trade-off because <additional rationale>.
 **/
"#
    );
}

#[test]
fn stripes_to_space() {
    harness!(
        Rust,
        r#"
//! YADR: 2023-11-30 bar-baz
//! In the context of <use case/user story u>, we faced <concern c>.
//!
//! We decided for <option o>, and neglected <other options>.
//!
//! We did this to achieve <system qualities/desired consequences>, accepting
//! <downside d/undesired consequences>.
//!
//! We think this is the right trade-off because <additional rationale>.
//!
//! some more additional docs
//! for this
//! module
//!
//!
//! YADR: 2023-12-01 foo-bar-baz
//! In the context of <use case/user story u>, we faced <concern c>.
//!
//! We decided for <option o>, and neglected <other options>.
//!
//! We did this to achieve <system qualities/desired consequences>, accepting
//! <downside d/undesired consequences>.
//!
//! We think this is the right trade-off because <additional rationale>.
"#
    );
}

#[test]
fn hashes() {
    harness!(
        Python,
        r#"
# YADR: 2023-11-30 bar-baz
# In the context of <use case/user story u>, we faced <concern c>.
#
# We decided for <option o>, and neglected <other options>.
#
# We did this to achieve <system qualities/desired consequences>, accepting
# <downside d/undesired consequences>.
#
# We think this is the right trade-off because <additional rationale>.
"#
    );
}

#[test]
fn docstr_double() {
    harness!(
        Python,
        r#"
"""
YADR: 2023-11-30 bar-baz
In the context of <use case/user story u>, we faced <concern c>.

We decided for <option o>, and neglected <other options>.

We did this to achieve <system qualities/desired consequences>, accepting
<downside d/undesired consequences>.

We think this is the right trade-off because <additional rationale>.
"""
"#
    );
}

#[test]
fn docstr_single() {
    harness!(
        Python,
        r#"
'''
YADR: 2023-11-30 bar-baz
In the context of <use case/user story u>, we faced <concern c>.

We decided for <option o>, and neglected <other options>.

We did this to achieve <system qualities/desired consequences>, accepting
<downside d/undesired consequences>.

We think this is the right trade-off because <additional rationale>.
'''
"#
    );
}

#[test]
fn docstr_raw() {
    harness!(
        Python,
        r#"
r'''
YADR: 2023-11-30 bar-baz
In the context of <use case/user story u>, we faced <concern c>.

We decided for <option o>, and neglected <other options>.

We did this to achieve <system qualities/desired consequences>, accepting
<downside d/undesired consequences>.

We think this is the right trade-off because <additional rationale>.
'''
"#
    );
}

#[test]
fn docstr_unicode() {
    harness!(
        Python,
        r#"
u'''
YADR: 2023-11-30 bar-baz
In the context of <use case/user story u>, we faced <concern c>.

We decided for <option o>, and neglected <other options>.

We did this to achieve <system qualities/desired consequences>, accepting
<downside d/undesired consequences>.

We think this is the right trade-off because <additional rationale>.
'''
"#
    );
}

#[test]
fn pytest_assertion_introspection() {
    harness!(
        Python,
        r#"
@pytest
def test__foo():
    assert false, "Running transform again must give different results."

@pytest
def test__foo():
    assert false, f"Running transform again must give different results."

@pytest
def test__foo():
    assert false, 'Running transform again must give different results.'

@pytest
def test__foo():
    assert false, rb'Running transform again must give different results.'

@pytest
def test__foo():
    assert false, RF"Running transform again must give different results."
"#
    );
}

#[test]
fn docstr_raw_unicode() {
    harness!(
        Python,
        r#"
ru'''
YADR: 2023-11-30 bar-baz
In the context of <use case/user story u>, we faced <concern c>.

We decided for <option o>, and neglected <other options>.

We did this to achieve <system qualities/desired consequences>, accepting
<downside d/undesired consequences>.

We think this is the right trade-off because <additional rationale>.
'''
"#
    );
}

#[test]
fn early_termination_dashes() {
    harness!(
        Rust,
        r#"
// YADR: 2023-11-30 bar-baz
// In the context of <use case/user story u>, we faced <concern c>.
//
// We decided for <option o>, and neglected <other options>.
//
// We did this to achieve <system qualities/desired consequences>, accepting
// <downside d/undesired consequences>.
//
// We think this is the right trade-off because <additional rationale>.
//
// Here's a tail.
//
// ---
//
// I don't want this included.
"#
    );
}

#[test]
fn early_termination_triple_newline() {
    harness!(
        Rust,
        r#"
// YADR: 2023-11-30 bar-baz
// In the context of <use case/user story u>, we faced <concern c>.
//
// We decided for <option o>, and neglected <other options>.
//
// We did this to achieve <system qualities/desired consequences>, accepting
// <downside d/undesired consequences>.
//
// We think this is the right trade-off because <additional rationale>.
//
// Here's a tail.
//
//
// I don't want this included.
"#
    );
}

#[test]
fn with_changes() {
    harness!(
        Rust,
        r#"
// YADR: 2023-11-30 bar-baz
// In the context of <use case/user story u>, we faced <concern c>.
//
// We decided for <option o>, and neglected <other options>.
//
// We did this to achieve <system qualities/desired consequences>, accepting
// <downside d/undesired consequences>.
//
// We think this is the right trade-off because <additional rationale>.
//
// Here's a tail.
//
// 2023-12-01: changed thing 1
// 2023-12-02: changed thing 2
// 2023-12-03: changed thing 3
"#
    );
}

#[test]
fn early_termination_with_changes() {
    harness!(
        Rust,
        r#"
// YADR: 2023-11-30 bar-baz
// In the context of <use case/user story u>, we faced <concern c>.
//
// We decided for <option o>, and neglected <other options>.
//
// We did this to achieve <system qualities/desired consequences>, accepting
// <downside d/undesired consequences>.
//
// We think this is the right trade-off because <additional rationale>.
//
// Here's a tail.
//
// 2023-12-01: changed thing 1
// 2023-12-02: changed thing 2
// 2023-12-03: changed thing 3
//
//
// I don't want this included.
"#
    );
}

/// Parses a Y-Statement taken verbatim from a real code base (the [`dson`] crate) rather than one
/// written to fit the parser, so that we notice if the format only works for prose we wrote
/// ourselves.
///
/// [`dson`]: https://github.com/helsing-ai/dson/blob/607ee3a8a7e9a56cb80746b66faa9e6283b5b276/src/crdts/orarray.rs#L434-L454
#[test]
fn realistic() {
    harness!(
        Rust,
        r#"
/*
 * YADR: 2024-06-18 Array elements without a position defined
 *
 * In the context of dealing with array elements which don't have a position defined,
 * we faced a decision of how to expose these elements through the public OrArray API.
 *
 * We decided for assigning these elements an arbitrary but deterministic position, as
 * a function of their uid, and neglected to attempt to map them to the start or end of
 * the array, or provide a separate API for access to position-less elements, when their
 * uid is not yet known.
 *
 * We did this to achieve minimal impact to the user-facing API, to avoid increasing the
 * cognitive burden of using this crate, and to ensure that every node has a consistent
 * view of the array when they share the same state, accepting that users may be
 * surprised to find that a non-move operation (like a delete) can result in an element
 * being assigned a different position.
 *
 * We think this is the right trade-off because this is a rare edge case, and placing
 * the burden of handling it on users (by providing a separate access interface)
 * would've been unreasonable. Additionally, non-deterministic views of the array would
 * have violated a core assumption that nodes in sync with each other have the same view
 * of the state.
 */
"#
    );
}

/// Guards the byte-offset arithmetic behind the "which paragraph is malformed" diagnostic.
///
/// Two properties matter here. The comment is *indented*, so each paragraph's offset has to
/// account for the leading whitespace on its first line; and the paragraphs ahead of the
/// malformed one span several lines each, so the offset also has to account for the line breaks
/// within them. Get either wrong and the span drifts off the paragraph it is meant to underline.
///
/// The third paragraph opens with "We did this to produce" where the format calls for "We did
/// this to achieve", and is otherwise well-formed, so the diagnostic should point at that
/// paragraph and only that paragraph.
#[test]
fn indented_malformed_paragraph() {
    harness!(
        Rust,
        r#"
        /*
         * YADR: 2024-02-10 bar-baz
         *
         * In the context of <use case/user story u> written out at enough length that it
         * wraps onto a second and then a third line of the comment, we faced <concern c>
         * that is described at similar length.
         *
         * We decided for <option o>, spelled out over several lines so that the paragraph
         * ahead of the malformed one is more than a single line tall, and neglected <other
         * options>, which are listed here at a comparable length so that this paragraph
         * ends up four lines tall in total.
         *
         * We did this to produce <system qualities/desired consequences>, spread over
         * enough lines that the reported span has to cover more than one of them, and
         * padded out further so that the shape of this paragraph matches the one before
         * it, accepting <downside d/undesired consequences>, which are written out at
         * length for the same reason.
         *
         * We think this is the right trade-off because <additional rationale>, stated over
         * a few lines so that the diagnostic has something to show underneath the span it
         * highlights.
         *
         * A trailing paragraph, so the malformed one is not the last thing in the comment.
         */
"#
    );
}

#[test]
fn two_in_one_rs() {
    harness!(
        Rust,
        r#"
/*
 * YADR: 2023-11-29 foo-bar also
 * In the context of <use case/user story u>, we faced <concern c>.
 *
 * We decided for <option o>, and neglected <other options>.
 *
 * We did this to achieve <system qualities/desired consequences>, accepting
 * <downside d/undesired consequences>.
 *
 * We think this is the right trade-off because <additional rationale>.

 YADR: 2023-11-30 bar-baz
 In the context of <use case/user story u>, we faced <concern c>.

 We decided for <option o>, and neglected <other options>.

 We did this to achieve <system qualities/desired consequences>, accepting
 <downside d/undesired consequences>.

 We think this is the right trade-off because <additional rationale>.
 */
"#
    );
}

#[test]
fn two_in_one_py() {
    harness!(
        Python,
        r#"
# YADR: 2023-11-29 foo-bar also
# In the context of <use case/user story u>, we faced <concern c>.
#
# We decided for <option o>, and neglected <other options>.
#
# We did this to achieve <system qualities/desired consequences>, accepting
# <downside d/undesired consequences>.
#
# We think this is the right trade-off because <additional rationale>.
#
# YADR: 2023-11-30 bar-baz
# In the context of <use case/user story u>, we faced <concern c>.
#
# We decided for <option o>, and neglected <other options>.
#
# We did this to achieve <system qualities/desired consequences>, accepting
# <downside d/undesired consequences>.
#
# We think this is the right trade-off because <additional rationale>.
"#
    );
}

#[test]
fn missing_paragraph() {
    harness!(
        Rust,
        r#"
/*
 * YADR: 2023-11-29 foo-bar also
 * In the context of <use case/user story u>, we faced <concern c>.
 *
 * We decided for <option o>, and neglected <other options>.
 *
 * We did this to achieve <system qualities/desired consequences>, accepting
 * <downside d/undesired consequences>.
 */
"#
    );
}

#[test]
fn malformed_para() {
    harness!(
        Rust,
        r#"
/*
 * YADR: 2023-11-29 foo-bar also
 * In the context of <use case/user story u>, we faced <concern c>.
 *
 * We failed to follow the format for <option o>, and neglected <other options>.
 *
 * We did this to achieve <system qualities/desired consequences>, accepting
 * <downside d/undesired consequences>.
 *
 * We think this is the right trade-off because <additional rationale>.
 */
"#
    );
}

#[test]
fn title_no_colon() {
    harness!(
        Rust,
        r#"
/*
 * YADR 2023-11-29 foo-bar also
 * In the context of <use case/user story u>, we faced <concern c>.
 *
 * We decided for <option o>, and neglected <other options>.
 *
 * We did this to achieve <system qualities/desired consequences>, accepting
 * <downside d/undesired consequences>.
 *
 * We think this is the right trade-off because <additional rationale>.
 */
"#
    );
}

#[test]
fn no_title_seq() {
    harness!(
        Rust,
        r#"
/*
 * YADR: foo-bar also
 * In the context of <use case/user story u>, we faced <concern c>.
 *
 * We decided for <option o>, and neglected <other options>.
 *
 * We did this to achieve <system qualities/desired consequences>, accepting
 * <downside d/undesired consequences>.
 *
 * We think this is the right trade-off because <additional rationale>.
 */
"#
    );
}

#[test]
fn no_title() {
    harness!(
        Rust,
        r#"
/*
 * YADR
 * In the context of <use case/user story u>, we faced <concern c>.
 *
 * We decided for <option o>, and neglected <other options>.
 *
 * We did this to achieve <system qualities/desired consequences>, accepting
 * <downside d/undesired consequences>.
 *
 * We think this is the right trade-off because <additional rationale>.
 */
"#
    );
}

#[test]
fn empty_title() {
    harness!(
        Rust,
        r#"
/*
 * YADR:   
 * In the context of <use case/user story u>, we faced <concern c>.
 *
 * We decided for <option o>, and neglected <other options>.
 *
 * We did this to achieve <system qualities/desired consequences>, accepting
 * <downside d/undesired consequences>.
 *
 * We think this is the right trade-off because <additional rationale>.
 */
"#
    );
}

#[test]
fn change_missing_colon() {
    harness!(
        Rust,
        r#"
// YADR: 2023-11-30 bar-baz
// In the context of <use case/user story u>, we faced <concern c>.
//
// We decided for <option o>, and neglected <other options>.
//
// We did this to achieve <system qualities/desired consequences>, accepting
// <downside d/undesired consequences>.
//
// We think this is the right trade-off because <additional rationale>.
//
// Here's a tail.
//
// 2023-12-01: changed thing 1
// 2023-12-02 changed thing 2
// 2023-12-03: changed thing 3
"#
    );
}

#[test]
fn changes_before_tail() {
    harness!(
        Rust,
        r#"
// YADR: 2023-11-30 bar-baz
// In the context of <use case/user story u>, we faced <concern c>.
//
// We decided for <option o>, and neglected <other options>.
//
// We did this to achieve <system qualities/desired consequences>, accepting
// <downside d/undesired consequences>.
//
// We think this is the right trade-off because <additional rationale>.
//
// 2023-12-01: changed thing 1
// 2023-12-02: changed thing 2
// 2023-12-03: changed thing 3
//
// Here's a tail.
"#
    );
}

#[test]
fn change_invalid_date() {
    harness!(
        Rust,
        r#"
// YADR: 2023-11-30 bar-baz
// In the context of <use case/user story u>, we faced <concern c>.
//
// We decided for <option o>, and neglected <other options>.
//
// We did this to achieve <system qualities/desired consequences>, accepting
// <downside d/undesired consequences>.
//
// We think this is the right trade-off because <additional rationale>.
//
// Here's a tail.
//
// 2023-12-01: changed thing 1
// 000: changed thing 2
// 2023-12-03: changed thing 3
"#
    );
}

#[test]
fn no_empty_line_before_changes() {
    harness!(
        Rust,
        r#"
// YADR: 2023-11-30 bar-baz
// In the context of <use case/user story u>, we faced <concern c>.
//
// We decided for <option o>, and neglected <other options>.
//
// We did this to achieve <system qualities/desired consequences>, accepting
// <downside d/undesired consequences>.
//
// We think this is the right trade-off because <additional rationale>.
//
// Here's a tail.
// 2023-12-01: changed thing 1
// 2023-12-02: changed thing 2
// 2023-12-03: changed thing 3
"#
    );
}

#[test]
fn bad_change_ordering() {
    harness!(
        Rust,
        r#"
// YADR: 2023-11-30 bar-baz
// In the context of <use case/user story u>, we faced <concern c>.
//
// We decided for <option o>, and neglected <other options>.
//
// We did this to achieve <system qualities/desired consequences>, accepting
// <downside d/undesired consequences>.
//
// We think this is the right trade-off because <additional rationale>.
//
// Here's a tail.
//
// 2023-12-02: changed thing 2
// 2023-12-03: changed thing 3
// 2023-12-01: changed thing 1
"#
    );
}
