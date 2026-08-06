//! Parsing logic for Y-Statement [ADRs] in source code comments.
//!
//! These ADRs take the form:
//!
//! ```rust
//! /*
//!  * YADR: 2023-11-28 some-title
//!  *
//!  * In the context of <use case/user story u>, we faced <concern c>.
//!  *
//!  * We decided for <option o>, and neglected <other options>.
//!  *
//!  * We did this to achieve <system qualities/desired consequences>, accepting <downside
//!  * d/undesired consequences>.
//!  *
//!  * We think this is the right trade-off because <additional rationale>.
//!  */
//! ```
//!
//! and should be used to *concisely* convey important architectural decisions in software
//! projects. For example, here is one taken from the [`dson`] crate:
//!
//! ```
//! /*
//!  * YADR: 2024-06-18 Array elements without a position defined
//!  *
//!  * In the context of dealing with array elements which don't have a position defined,
//!  * we faced a decision of how to expose these elements through the public OrArray API.
//!  *
//!  * We decided for assigning these elements an arbitrary but deterministic position, as
//!  * a function of their uid, and neglected to attempt to map them to the start or end of
//!  * the array, or provide a separate API for access to position-less elements, when their
//!  * uid is not yet known.
//!  *
//!  * We did this to achieve minimal impact to the user-facing API, to avoid increasing the
//!  * cognitive burden of using this crate, and to ensure that every node has a consistent
//!  * view of the array when they share the same state, accepting that users may be
//!  * surprised to find that a non-move operation (like a delete) can result in an element
//!  * being assigned a different position.
//!  *
//!  * We think this is the right trade-off because this is a rare edge case, and placing
//!  * the burden of handling it on users (by providing a separate access interface)
//!  * would've been unreasonable. Additionally, non-deterministic views of the array would
//!  * have violated a core assumption that nodes in sync with each other have the same view
//!  * of the state.
//!  */
//! ```
//!
//! Note that we diverge slightly from [the original Y-Statement format][ystmt] to make the comment
//! easier to digest!
//!
//! # Errors
//!
//! Everything here reports failures as [`miette::Report`] rather than as a bespoke error enum,
//! which is a deliberate departure from the usual advice to expose a concrete, matchable error
//! type.
//!
//! The reason is that a failure here is a *diagnostic*, not a condition to branch on. Each one
//! carries the span of the offending text, a suggestion for how to fix it, and the surrounding
//! source, so that a caller can render something a human can act on. That is the entire value of
//! the error, and there is very little a caller could sensibly do differently on the strength of
//! knowing which paragraph failed to parse. An enum would either have to throw that information
//! away or wrap it, and callers would be no better off.
//!
//! [ADRs]: https://adr.github.io/
//! [ystmt]: https://medium.com/olzzio/y-statements-10eb07b5a177
//! [`dson`]: https://github.com/helsing-ai/dson/blob/607ee3a8a7e9a56cb80746b66faa9e6283b5b276/src/crdts/orarray.rs#L434-L454

// allow comment-only Rust code blocks
#![allow(rustdoc::invalid_rust_codeblocks)]
#![deny(missing_docs, missing_debug_implementations)]

use jiff::civil::Date;
use miette::{miette, IntoDiagnostic, LabeledSpan, WrapErr};
use ra_ap_syntax::Edition;
use ra_ap_syntax::{ast::SourceFile, NodeOrToken, SyntaxKind, WalkEvent};
use regex::Regex;
use std::sync::LazyLock;
use std::{borrow::Cow, fmt};
use streaming_iterator::StreamingIterator;
use tree_sitter::Parser;

static CAPTURE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    let mut actual_regex = String::from(
        r"(?xs)
        # allow leading whitespace
        ^[[:space:]]*",
    );
    for (i, (_, rx)) in YADR_PARAGRAPH_REGEXES.iter().enumerate() {
        if i != 0 {
            // newlines to make the regex easier to read if printed
            actual_regex.push('\n');
            actual_regex.push('\n');
            // paragraph break
            actual_regex.push_str(r"([[:blank:]]*\n){2}[[:blank:]]*");
            actual_regex.push('\n');
        }
        actual_regex.push_str(rx);
    }
    Regex::new(&actual_regex).expect("valid regex")
});
static SPLIT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^[[:blank:]]*YADR\b").expect("valid regex"));
static TRIM_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^[[:blank:]]*(---+|\n[[:blank:]]*$)").expect("valid regex"));
static BLOCK_COMMENT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*\*").expect("regex to be valid"));
/// Matches the paragraph break that `CAPTURE_REGEX` expects between Y-Statement paragraphs.
///
/// Only used by [`better_error`], which walks the paragraphs one at a time to work out which of
/// them is the one that didn't match.
static PARAGRAPH_SPLIT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"([[:blank:]]*\n){2}[[:blank:]]*").expect("valid regex"));

// this is really one giant regex (see `let regex =` in `parse_yadr_block`), but is split so that
// we can produce better error messages (see `better_error`).
//
// for each of these regexes, note that every gap between words _could_ be at a line-break in
// the comment, so we match each one with any 1+ whitespace characters (which includes
// newlines)
//
// we match each groups with
//  - ?P<name> to get a named capture,
//  - .+ since the group should never be empty
//    note that the preceding [[:space:]]+ has gobbled up any leading whitespace
const YADR_PARAGRAPH_REGEXES: &[(&str, &str)] = &[
    (
        "\"In the context of <..>, we faced <..>.\"",
        r"
        In[[:space:]]+the[[:space:]]+context[[:space:]]+of[[:space:]]+(?P<in_context>.+?),[[:space:]]+
        we[[:space:]]+faced[[:space:]]+(?P<facing_concern>.+?)\.",
    ),
    (
        "\"We decided for <..>, and neglected <..>.\"",
        r"
        We[[:space:]]+decided[[:space:]]+for[[:space:]]+(?P<chosen_option>.+?),[[:space:]]+
        and[[:space:]]+neglected[[:space:]]+(?P<neglected_options>.+?)\.",
    ),
    (
        "\"We did this to achieve <..>, accepting <..>.\"",
        r"
        We[[:space:]]+did[[:space:]]+this[[:space:]]+to[[:space:]]+achieve[[:space:]]+(?P<to_achieve>.+?),
        [[:space:]]+accepting[[:space:]]+(?P<accepted_downsides>.+?)\.",
    ),
    // match last group with ? so that it doesn't end up eating the tail (if any)
    // the Y-Statement ends either at end of comment or double-newline, whichever comes first.
    // if it's a double-newline, capture the tail too.
    (
        "\"We think this is the right trade-off because <..>.\"",
        r"
        We[[:space:]]+think[[:space:]]+this[[:space:]]+is[[:space:]]+the[[:space:]]+
        right[[:space:]]+trade-off[[:space:]]+because[[:space:]]+(?P<because>.+?)\.
        (?:[[:space:]]*$|([[:blank:]]*\n){2}(?P<tail>.+))",
    ),
];

/// The language of a source file.
///
/// Used to determine how to parse a given input `&str`.
///
/// This enum is `#[non_exhaustive]`: support for further languages is expected, and adding a
/// variant shouldn't be a breaking change. Match on it with a `_` arm, and reach for
/// [`Language::ALL`] if you need to enumerate what this version supports.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Language {
    /// `.rs`
    Rust,
    /// `.py`
    Python,
    /// `.nix`
    Nix,
}

impl Language {
    /// Every language this version of `yadr` can parse.
    pub const ALL: &'static [Language] = &[Language::Rust, Language::Python, Language::Nix];

    /// Returns the language conventionally written in files with the given extension.
    ///
    /// The extension is given without a leading `.`, matching
    /// [`Path::extension`](std::path::Path::extension). Returns `None` for anything this version
    /// of `yadr` can't parse, which is the common case when walking a source tree.
    ///
    /// ```
    /// use yadr::Language;
    ///
    /// assert_eq!(Language::from_extension("rs"), Some(Language::Rust));
    /// assert_eq!(Language::from_extension("md"), None);
    /// ```
    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension {
            "rs" => Some(Language::Rust),
            "py" => Some(Language::Python),
            "nix" => Some(Language::Nix),
            _ => None,
        }
    }

    /// Returns the file extension conventionally used for this language, without a leading `.`.
    ///
    /// Where a language has more than one extension in common use, this is the canonical one, so
    /// `Language::from_extension(l.extension()) == Some(l)` holds but the reverse does not.
    pub fn extension(self) -> &'static str {
        match self {
            Language::Rust => "rs",
            Language::Python => "py",
            Language::Nix => "nix",
        }
    }

    /// Returns the name of this language, as accepted by its [`FromStr`](std::str::FromStr) impl.
    pub fn name(self) -> &'static str {
        match self {
            Language::Rust => "rust",
            Language::Python => "python",
            Language::Nix => "nix",
        }
    }
}

impl fmt::Display for Language {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl std::str::FromStr for Language {
    type Err = UnknownLanguage;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Language::ALL
            .iter()
            .copied()
            .find(|language| language.name().eq_ignore_ascii_case(s))
            .ok_or_else(|| UnknownLanguage(s.to_string()))
    }
}

/// The error returned when a string does not name a [`Language`] that `yadr` can parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownLanguage(String);

impl UnknownLanguage {
    /// Returns the string that failed to parse.
    pub fn unrecognized(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for UnknownLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown language '{}', expected one of ", self.0)?;
        for (i, language) in Language::ALL.iter().enumerate() {
            if i != 0 {
                f.write_str(", ")?;
            }
            write!(f, "'{language}'")?;
        }
        Ok(())
    }
}

impl std::error::Error for UnknownLanguage {}

/// Invokes `on_yadr` for each Y-Statement ADR contained in source code comments in `input`.
///
/// `on_yadr` is passed the line number of the title line of each ADR, as well as a [`YAdr`]
/// that holds the information parsed out of the ADR block.
///
/// Stops searching early if `on_yadr` returns `Ok(false)`, or if an error is encountered.
///
/// # Why a callback and not an iterator
///
/// An iterator would read better, and for Rust input it would even be possible, since each
/// [`YAdr`] borrows directly out of `input`. It isn't possible for the other languages: those go
/// through tree-sitter, which hands back comment text that has to be stripped of its `#` or `*`
/// prefixes first, so each [`YAdr`] borrows from a temporary the traversal owns. An iterator
/// would have to yield items borrowing from itself, which Rust cannot express, so the choice is
/// between a callback and allocating an owned [`YAdr`] for every statement in the tree.
///
/// If you want to collect the results anyway, [`YAdr::into_owned`] cuts the borrow:
///
/// ```
/// use yadr::{find_all, Language, YAdr};
///
/// # let source = std::fs::read_to_string(file!()).unwrap();
/// let mut found: Vec<(usize, YAdr<'static>)> = Vec::new();
/// find_all(&source, Language::Rust, |line, yadr| {
///     found.push((line, yadr.into_owned()));
///     Ok(true)
/// })?;
/// # Ok::<_, miette::Report>(())
/// ```
pub fn find_all(
    input: &str,
    language: Language,
    mut on_yadr: impl FnMut(usize, YAdr<'_>) -> miette::Result<bool>,
) -> miette::Result<()> {
    let on = |line: usize, date: Date, title: &str, body: &str| {
        let yadr = parse_yadr_block(date, title, body)
            .wrap_err_with(|| format!("in Y-Statement starting on line {line}"))?;
        on_yadr(line, yadr)
    };

    match language {
        Language::Rust => find_yadr_sections_rs(input, on),
        _ => find_yadr_sections_tree_sitter(input, language, on),
    }
}

/// Calls `on_comment_end` with `on_yadr` for each comment block in `input`.
///
/// A comment block is any block comment (ie, /**/) or any consecutive sequence of //-prefixed
/// or //!-prefixed lines.
///
/// Stops searching if `on_yadr` returns `Ok(false)`, or if an error is encountered.
fn find_yadr_sections_rs(
    input: &str,
    mut on_yadr: impl FnMut(usize, Date, &str, &str) -> miette::Result<bool>,
) -> miette::Result<()> {
    // picking an edition is difficult here since we don't want to start understanding Cargo
    // workspaces. we could theoretically run
    //
    //     cargo metadata --format-version=1 --no-deps \
    //       | jq -r '.packages[] | select(.id == "$(cargo pkgid)").edition'
    //
    // at $(dirname "$filename") for each parsed file, but that'd add significant overhead, not to
    // mention we would have to pass the file path (or, more likely, detected edition) all the way
    // down to here even though it doesn't make sense for other languages.
    //
    // so, we instead just use the most recent Rust version, which should be able to parse _most_
    // Rust files in practice.
    let parse = SourceFile::parse(input, Edition::CURRENT);
    let mut consecutive_comments = None;
    let mut was_whitespace = false;
    let mut line_at_byte = (0, 1);
    for token in parse.syntax_node().preorder_with_tokens() {
        if let WalkEvent::Leave(_) = token {
            continue;
        }
        let WalkEvent::Enter(NodeOrToken::Token(token)) = token else {
            if !on_comment_end(&mut consecutive_comments, &mut on_yadr)? {
                break;
            }
            continue;
        };
        match token.kind() {
            SyntaxKind::COMMENT => {
                let start: usize = token.text_range().start().into();
                // NOTE: .split('\n') rather than .lines(), since .lines() special-cases trailing newline
                let start_line = line_at_byte.1
                    + input[line_at_byte.0..start]
                        .split('\n')
                        .count()
                        .saturating_sub(1);
                line_at_byte = (start, start_line);

                let text = token.text();
                if text.starts_with("///") || text.starts_with("/**") {
                    // is a doc comment
                    if !on_comment_end(&mut consecutive_comments, &mut on_yadr)? {
                        break;
                    }
                    continue;
                }

                was_whitespace = false;
                if let Some(text) = text.strip_prefix("/*") {
                    if !on_comment_end(&mut consecutive_comments, &mut on_yadr)? {
                        break;
                    }
                    let mut out = String::new();
                    let text = text.strip_suffix("*/").expect("/* ends with */");
                    for line in text.lines() {
                        let line = line.trim_start();
                        let line = line.strip_prefix('*').unwrap_or(line);
                        let line = line.trim_start();
                        out.push_str(line);
                        out.push('\n');
                    }
                    consecutive_comments = Some((start_line, out));
                    if !on_comment_end(&mut consecutive_comments, &mut on_yadr)? {
                        break;
                    }
                } else if let Some(text) = text.strip_prefix("//!") {
                    if let Some((_, adr)) = &mut consecutive_comments {
                        adr.push_str(text.trim_start());
                    } else {
                        consecutive_comments = Some((start_line, String::from(text.trim_start())));
                    }
                } else if let Some(text) = text.strip_prefix("//") {
                    if let Some((_, adr)) = &mut consecutive_comments {
                        adr.push_str(text.trim_start());
                    } else {
                        consecutive_comments = Some((start_line, String::from(text.trim_start())));
                    }
                } else {
                    unreachable!("there are no other types of comments?");
                }
            }
            SyntaxKind::WHITESPACE if consecutive_comments.is_some() => {
                if was_whitespace {
                    if !on_comment_end(&mut consecutive_comments, &mut on_yadr)? {
                        break;
                    }
                } else {
                    let (_, cc) = consecutive_comments.as_mut().expect("is_some in arm");
                    let (just_newline, trailing_ws) = token.text().split_once('\n').unwrap_or({
                        // hit EOF, so there's nothing more
                        ("", "")
                    });
                    assert!(just_newline.is_empty());
                    cc.push_str(just_newline);
                    cc.push('\n');
                    if trailing_ws.contains('\n') {
                        // an empty line! that ends the comment sequence
                        if !on_comment_end(&mut consecutive_comments, &mut on_yadr)? {
                            break;
                        }
                    } else {
                        // some whitespace before the next // or /* is fine
                    }
                    was_whitespace = true;
                }
            }
            SyntaxKind::WHITESPACE => {
                was_whitespace = true;
            }
            _ => {
                if !on_comment_end(&mut consecutive_comments, &mut on_yadr)? {
                    break;
                }
                was_whitespace = false;
            }
        }
    }
    on_comment_end(&mut consecutive_comments, &mut on_yadr)?;

    Ok(())
}

/// Calls `on_comment_end` with `on_yadr` for each comment block in `input`.
///
/// A comment block is any block comment (ie, /**/) or any consecutive sequence of //-prefixed
/// lines.
///
/// Stops searching if `on_yadr` returns `Ok(false)`, or if an error is encountered.
fn find_yadr_sections_tree_sitter(
    input: &str,
    language: Language,
    mut on_yadr: impl FnMut(usize, Date, &str, &str) -> miette::Result<bool>,
) -> miette::Result<()> {
    use tree_sitter::{Query, QueryCapture, QueryCursor};

    let ts_language = match language {
        Language::Python => tree_sitter_python::LANGUAGE.into(),
        Language::Nix => tree_sitter_nix::LANGUAGE.into(),
        Language::Rust => unreachable!("`find_yadr_sections_tree_sitter` never called with rust."),
    };
    let mut parser = Parser::new();
    parser
        .set_language(&ts_language)
        .into_diagnostic()
        .wrap_err("loading language grammar")?;
    let Some(parsed) = parser.parse(input, None) else {
        miette::bail!("parsing timed out or was cancelled");
    };
    let root = parsed.root_node();
    let query = match language {
        Language::Python => Query::new(
            &ts_language,
            "([(comment)+ @comments (expression_statement (string) @docstr)])",
        ),
        Language::Nix => Query::new(&ts_language, "((comment)+ @comments)"),
        Language::Rust => unreachable!("`find_yadr_sections_tree_sitter` never called with rust."),
    }
    .into_diagnostic()
    .wrap_err("tree-sitter comment query syntax check")?;
    let mut cursor = QueryCursor::new();
    let mut captures = cursor.captures(&query, root, input.as_bytes());

    let mut consecutive_comments = None;
    'all: while let Some((capture, _)) = captures.next() {
        let captured = capture.captures;
        let mut last_line = 0;
        for QueryCapture { node, index } in captured {
            let mut text = node
                .utf8_text(input.as_bytes())
                .expect("input is str, so must be valid utf-8");
            let text = match index {
                0 => {
                    /* @comment */
                    if let Some(text) = text.strip_prefix("#") {
                        // line comment e.g. Python / Nix
                        text.trim_start()
                    } else if let Some(text) = text.strip_prefix("/*") {
                        // Block comment

                        // This uses ?m multineline mode to have ^ match after newlines
                        &BLOCK_COMMENT_REGEX.replace_all(
                            text.strip_suffix("*/")
                                .expect("block comments end with */")
                                .trim_end_matches("*"),
                            "",
                        )
                    } else {
                        miette::bail!("Found unexpected prefix in text {}", text)
                    }
                }
                1 => {
                    // @docstr
                    //
                    // note, however, that we're just matching for _strings_ here, which means this
                    // can capture all sorts of strings, not just specifically docstrings. most
                    // strings in Python don't immediately follow an expression, but some can,
                    // such as pytest assertion introspection statements:
                    //
                    //   https://docs.pytest.org/en/8.0.x/how-to/assert.html#assertion-introspection-details
                    //
                    // so, we specifically have to check that we are indeed encountering a
                    // docstring below, not just assume that we are!
                    loop {
                        match text
                            .chars()
                            .next()
                            .expect("delimiters are included, so must be non-empty")
                        {
                            '"' => {
                                let Some(text) = text.strip_prefix("\"\"\"") else {
                                    // this means we're running into the
                                    // string-following-expression case mentioned above, which
                                    // means it's _not_ a docstring, so we can safely ignore it.
                                    break "";
                                };
                                break text
                                    .strip_suffix("\"\"\"")
                                    .expect("Python doc blocks end with \"\"\"");
                            }
                            '\'' => {
                                let Some(text) = text.strip_prefix("'''") else {
                                    // as-above
                                    break "";
                                };
                                break text
                                    .strip_suffix("'''")
                                    .expect("Python doc blocks end with '''");
                            }
                            // docstrings only allow the r and u modifiers as per
                            // https://peps.python.org/pep-0257/#specification
                            c @ 'r' | c @ 'u' => {
                                // raw or unicode docstrings
                                text = text
                                    .strip_prefix(c)
                                    .expect("we *just* found that it started with this");
                                continue;
                            }
                            // other "special" string types that are _not_ docstring-compatible.
                            // we can safely assume these don't hold Y-ADRs.
                            // ref https://docs.python.org/3/reference/lexical_analysis.html#string-and-bytes-literals
                            _ => {
                                break "";
                            }
                        }
                    }
                }
                _ => unreachable!("only two patterns in input"),
            };

            let start_line = node.start_position().row + 1;
            #[allow(clippy::collapsible_if)]
            if *index == 1 || last_line + 1 != start_line {
                if !on_comment_end(&mut consecutive_comments, &mut on_yadr)? {
                    break 'all;
                }
            }
            last_line = node.end_position().row + 1;

            if *index == 1 {
                consecutive_comments = Some((start_line, text.to_string()));
                if !on_comment_end(&mut consecutive_comments, &mut on_yadr)? {
                    break 'all;
                }
                continue;
            }

            if let Some((_, adr)) = &mut consecutive_comments {
                adr.push_str(text);
                adr.push('\n');
            } else {
                let mut text = String::from(text);
                text.push('\n');
                consecutive_comments = Some((start_line, text));
            }
        }

        if !on_comment_end(&mut consecutive_comments, &mut on_yadr)? {
            break 'all;
        }

        capture.remove();
    }
    on_comment_end(&mut consecutive_comments, &mut on_yadr)?;

    Ok(())
}

/// Calls `on_comment` with `on_yadr` if `consecutive_comment.take()` is `Some`.
///
/// Called when a block of related comments ends (eg, either /**/ or consecutive // lines).
///
/// The `Option` part is just to make calling this function less annoying.
///
/// Returns `Ok(false)` if and only if `on_yadr` returns `Ok(false)`.
fn on_comment_end(
    consecutive_comment: &mut Option<(usize, String)>,
    on_yadr: &mut impl FnMut(usize, Date, &str, &str) -> miette::Result<bool>,
) -> miette::Result<bool> {
    if let Some((line, comment)) = consecutive_comment.take() {
        let comment = comment.trim_end_matches('\n');
        on_comment(line, comment, on_yadr).wrap_err_with(|| format!("in comment on line {line}"))
    } else {
        Ok(true)
    }
}

/// Calls `on_yadr` for each found Y-Statement ADR block within the provided comment.
///
/// This code considers any line that begins with `YADR` and is preceded by an empty line the start
/// of a Y-Statement ADR block. The block ends when another block is found, or when the comment
/// ends.
///
/// `on_yadr` is called with the starting line number of each Y-Statement ADR block (ie, the line
/// number of the YADR line), the sequence number at the beginning of the ADR title, the full ADR
/// title (including the sequence number), and the remainder of the block.
///
/// Returns `Ok(false)` if and only if `on_yadr` returns `Ok(false)`.
///
/// Note that the title and body handed to `on_yadr` borrow from `comment` for as long as `comment`
/// itself lives, rather than only for the duration of the call. [`YAdr::parse`] relies on that to
/// hand back a borrowing [`YAdr`]; the callers that walk a whole file don't care either way, since
/// the comment text they pass in is a temporary they own.
fn on_comment<'a>(
    mut line: usize,
    comment: &'a str,
    on_yadr: &mut impl FnMut(usize, Date, &'a str, &'a str) -> miette::Result<bool>,
) -> miette::Result<bool> {
    // NOTE: we don't include the : after YADR here so that we can detect user errors where they
    // leave it off this means that the contents of a YADR cannot contain a line that starts with
    // YADR. That's probably fine.
    let mut split = SPLIT_REGEX.split(comment);

    // whatever comes before the first YADR isn't interesting, so trim it out
    let mut pre_was_blank = true;
    if let Some(pre) = split.next() {
        line += pre.split('\n').count().saturating_sub(1);
        // we want to observe the difference between:
        //
        // // and so I wrote a
        // // YADR for this.
        //
        // and
        //
        // // so therefore:
        // //
        // // YADR foo bar
        //
        // the former isn't a YADR, the latter is an attempt at one with a syntax error (missing :)
        //
        // to distinguish them, we look at whether the last line of `pre` (discounting the trailing
        // \n) is empty. this isn't _perfect_, but it errs on the side of _not_ providing spurious
        // errors.
        let mut last_lines = pre.rsplit('\n');
        if let Some("") = last_lines.next() {
            pre_was_blank = last_lines.next().is_none_or(|line| line.trim().is_empty());
        } else {
            // this means that no YADR was found in the input!
            // this is a quirk of how regex splitting works, in that it yields sequences of the
            // input that _don't_ match the split string.
            assert_eq!(split.next(), None);
        }
    }
    for part in split {
        // compute the next line offset at the start of the loop so that `continue` works
        let this_line = line;
        line += part.split('\n').count().saturating_sub(1);

        let (title_line, post) = part.split_once('\n').unwrap_or((part, ""));
        let title = title_line.trim();
        let Some(title) = title.strip_prefix(':') else {
            if pre_was_blank {
                let help = if title.is_empty() {
                    String::from("try `YADR: 2023-11-28 some-concise-title`")
                } else {
                    format!("try `YADR:{title_line}`")
                };
                return Err(miette! {
                    help = help,
                    labels = vec![LabeledSpan::at(0..4 + title_line.len().min(1), "expected YADR:")],
                    "YADR statement on line {this_line} not followed by :"
                }
                .with_source_code(format!("YADR{title_line}")));
            } else {
                // probably not actually a YADR block.
                continue;
            }
        };
        let title = title.trim_start();
        if title.is_empty() {
            return Err(miette! {
                help = "try `YADR: 2023-11-28 some-concise-title`",
                labels = vec![LabeledSpan::at(0..4 + title_line.len(), "expected a title after the :")],
                "YADR statement on line {this_line} does not have a title"
            }
            .with_source_code(format!("YADR{title_line}")));
        }

        let date = title
            .split_whitespace()
            .next()
            .expect("split always yields once");
        let date: Date = date.parse().map_err(|e| {
            miette! {
                labels = vec![LabeledSpan::at(6..6 + date.len(), "expected a valid date")],
                help = format!("Try `YADR: YYYY-MM-DD {title}"),
                "YADR statement on line {this_line} has an invalid date: {e}"
            }
            .with_source_code(format!("YADR: {title}"))
        })?;

        // this regex splits the input string on one of two patterns:
        //
        // 1. lines that contain a `---` (or `----` or longer), and
        // 2. two consecutive empty lines.
        //
        // we then trim the yadr content such that it stops at the first such split (if any).
        // this is so that authors have a way to signal the end of a YADR without ending the
        // comment block. this can be useful if the YADR is part of documentation comment, or
        // otherwise appears in the middle of a longer explanatory comment. we allow both --- and
        // empty lines so that authors can choose whether it makes more sense for the Markdown
        // rendering of their comment to include a horizontal rule or not.
        let post = TRIM_REGEX
            .split(post)
            .next()
            .expect("split always yields once");

        if !on_yadr(this_line, date, title, post)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Attempts to parse `comment` as a Y-Statement ADR block.
fn parse_yadr_block<'a>(date: Date, title: &'a str, comment: &'a str) -> miette::Result<YAdr<'a>> {
    let Some(matches) = CAPTURE_REGEX.captures(comment) else {
        return Err(better_error(comment));
    };

    let tail = matches
        .name("tail")
        .map(|c| c.as_str())
        .filter(|c| !c.trim().is_empty());

    let mut changes = Vec::new();
    let tail = if let Some(tail) = tail {
        let mut saw_change_end = false;
        let mut is_bottommost_line = true;
        let mut start_of_changes = tail.len();
        let mut line_end = comment.len();
        for line in tail.lines().rev() {
            let line_start = line_end - line.len();
            line_end = line_end.saturating_sub(line.len() + 1);
            if line.trim().is_empty() {
                if !changes.is_empty() {
                    start_of_changes = start_of_changes.saturating_sub(line.len() + 1);
                    saw_change_end = true;
                }
                continue;
            }

            if let Some((maybe_date, change)) = line.trim_start().split_once(':') {
                let date_start = line_start + (line.len() - line.trim_start().len());
                match maybe_date.parse::<Date>() {
                    Ok(_) if saw_change_end => {
                        return Err(miette! {
                                labels = vec![LabeledSpan::at(line_start..line_start + line.len(), String::from("this looks like an ADR change entry"))],
                                help = "move the change entries to the end of the YADR block",
                                "found ADR change in the middle of a YADR block"
                            }
                            .with_source_code(String::from(comment)));
                    }
                    Ok(date) => {
                        start_of_changes = start_of_changes.saturating_sub(line.len() + 1);
                        if let Some((last, _)) = changes.first() {
                            if *last < date {
                                return Err(miette! {
                                    labels = vec![LabeledSpan::at(line_start..line_start + line.len(), String::from("this entry should come after the one below it"))],
                                    help = "sort the change entries such that they are listed from oldest to newest",
                                    "ADR change entries are not in chronological order"
                                }
                                .with_source_code(String::from(comment)));
                            }
                        }
                        changes.push((date, Cow::Borrowed(change.trim_start())))
                    }
                    Err(_) if is_bottommost_line => {
                        // this is the bottom-most non-empty line, and it doesn't look like a
                        // change entry, so there probably aren't any change entries
                        saw_change_end = true;
                    }
                    Err(e) if !saw_change_end => {
                        return Err(miette! {
                                labels = vec![LabeledSpan::at(date_start..date_start + maybe_date.len(), String::from("expected YYYY-MM-DD"))],
                                "Y-Statement change entry did not match expected format: {e:?}"
                            }
                            .with_source_code(String::from(comment)));
                    }
                    Err(_) => {
                        // this is just some random line with a : in it
                        saw_change_end = true;
                    }
                }
            } else if is_bottommost_line {
                saw_change_end = true;
            } else if !saw_change_end {
                // this is a non-change line that immediately precedes a change line
                // we require an empty line in between
                return Err(miette! {
                    labels = vec![LabeledSpan::at(line_start + line.len() + 1..comment.len(), String::from("before this list of changes"))],
                    "missing empty line before list of change list"
                }
                .with_source_code(String::from(comment)));
            }

            is_bottommost_line = false;
        }

        // we've been iterating over the lines in reverse order

        changes.reverse();
        let tail = &tail[..start_of_changes];
        if !tail.trim().is_empty() {
            Some(tail.trim())
        } else {
            None
        }
    } else {
        None
    };

    // every one of these groups sits outside any `?`-quantified part of `CAPTURE_REGEX`, so a
    // successful match (which we have, or we'd have returned above) always fills all of them.
    let group = |name: &str| {
        matches
            .name(name)
            .expect("group is mandatory in CAPTURE_REGEX")
            .as_str()
    };

    Ok(YAdr {
        date,
        title: Cow::Borrowed(title),
        in_context: group("in_context").into(),
        facing_concern: group("facing_concern").into(),
        chosen_option: group("chosen_option").into(),
        neglected_options: group("neglected_options").into(),
        to_achieve: group("to_achieve").into(),
        accepted_downsides: group("accepted_downsides").into(),
        because: group("because").into(),
        tail: tail.map(Into::into),
        changes,
    })
}

/// Produces a more specific Y-Statement ADR parsing error than "regular expression didn't match"
/// when possible.
fn better_error(comment: &str) -> miette::Report {
    // the initial match is all-or-nothing.
    // try to be more helpful with exactly what went wrong by matching paragraph-at-a-time.
    let mut byte_offset = 0;
    let paragraphs =
        PARAGRAPH_SPLIT_REGEX.splitn(comment.trim_start(), YADR_PARAGRAPH_REGEXES.len());
    let mut saw_n_valid = 0;
    for ((part, rx), paragraph) in YADR_PARAGRAPH_REGEXES.iter().zip(paragraphs) {
        saw_n_valid += 1;

        let we_start_at = if saw_n_valid == 1 {
            byte_offset += paragraph.len();
            0
        } else {
            // byte_offset is one past the last byte index of the previous paragraph.
            // line_offset is the line index of the last line of the previous paragraph.
            // we split by a double linebreak, so this paragraph starts at line_offset + 2
            // to figure out this paragraph's byte offset, we need to compute the number of
            // bytes between the end of the old paragraph and the start of this one. that, in
            // turn, is:
            let mut lines = comment[byte_offset..].split('\n');
            let trailing_space_of_previous = lines.next().expect("saw \\n\\n").len();
            let empty_line = lines.next().expect("saw \\n\\n").len();
            let start_of_this = lines.next().expect("saw \\n\\n");
            let leading_space_of_this = start_of_this.len() - start_of_this.trim_start().len();
            let our_byte_offset = byte_offset + trailing_space_of_previous + 1 /* \n */ + empty_line + 1 /* \n */ + leading_space_of_this;
            byte_offset = our_byte_offset + paragraph.len();
            our_byte_offset
        };

        // the paragraph should match completely, hence ^ and $.
        // NOTE: this won't catch cases where people don't use double-newlines for paragraphs.
        let partr = Regex::new(&format!("^(?xs){rx}$")).expect("valid regex");
        if partr.find(paragraph).is_some() {
            // this paragraph matches the expected format. yay!
            continue;
        }

        let help = if !paragraph.ends_with('.') {
            "Note the lack of a trailing `.`"
        } else {
            "Note that punctuation is significant; you have to match every , and ."
        };

        return miette! {
                labels = vec![LabeledSpan::at(we_start_at..we_start_at + paragraph.len(), format!("expected {part}"))],
                help = help,
                "Y-Statement did not match expected format"
            }
            .with_source_code(String::from(comment))
            .wrap_err(format!("in paragraph {saw_n_valid} of that statement"));
    }

    if saw_n_valid != YADR_PARAGRAPH_REGEXES.len() {
        let nparagraphs = match YADR_PARAGRAPH_REGEXES.len() - saw_n_valid {
            0 => unreachable!(),
            1 => String::from("a paragraph"),
            2 => String::from("two paragraphs"),
            n => format!("multiple ({n}) paragraphs"),
        };
        return miette! {
                labels = vec![LabeledSpan::at(comment.len() + 1..comment.len() + 4, format!("expected {}", YADR_PARAGRAPH_REGEXES[saw_n_valid].0))],
                "Y-Statement is missing {nparagraphs}.", 
            }
            .with_source_code(format!("{comment}\n   "));
    }

    // we tried really hard, but we can't pinpoint anything more specific about what's wrong
    miette! {
        "Y statement did not match expected format."
    }
    .with_source_code(String::from(comment))
}

/// A parsed Y-Statement ADR.
///
/// To pull these out of a source file, use [`find_all`]. To parse a single statement you already
/// have in hand, use [`YAdr::parse`] or its [`FromStr`](std::str::FromStr) impl.
///
/// These statements take the form:
///
/// ```rust
/// /*
///  * YADR: 2023-11-28 some-title
///  *
///  * In the context of <use case/user story u>, we faced <concern c>.
///  *
///  * We decided for <option o>, and neglected <other options>.
///  *
///  * We did this to achieve <system qualities/desired consequences>, accepting <downside
///  * d/undesired consequences>.
///  *
///  * We think this is the right trade-off because <additional rationale>.
///  */
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YAdr<'a> {
    /// The original authoring date of the ADR.
    pub date: Date,

    /// The title of this ADR.
    ///
    /// This includes the sequence number.
    pub title: Cow<'a, str>,

    /// The use case or user story that this ADR targets.
    pub in_context: Cow<'a, str>,

    /// The concerns faced by the authors that required a decision to be taken.
    pub facing_concern: Cow<'a, str>,

    /// The option ultimately taken by the ADR authors.
    pub chosen_option: Cow<'a, str>,

    /// The options considered but *not* taken by the ADR authors.
    pub neglected_options: Cow<'a, str>,

    /// The positive outcomes the chosen option optimizes for.
    pub to_achieve: Cow<'a, str>,

    /// The acknowledged downsides of the chosen option.
    pub accepted_downsides: Cow<'a, str>,

    /// The rationale for why this set of positives/downsides are the right trade-off.
    pub because: Cow<'a, str>,

    /// Any additional context provided by the ADR authors.
    pub tail: Option<Cow<'a, str>>,

    /// Logged changes to the ADR over time.
    pub changes: Vec<(Date, Cow<'a, str>)>,
}

impl<'a> YAdr<'a> {
    /// Parses a single Y-Statement, starting at its `YADR:` title line.
    ///
    /// Use this when you already have a statement in hand — because you pulled it out of a file
    /// format `yadr` doesn't know how to read, say — rather than a whole source file. To scan a
    /// source file, use [`find_all`] instead.
    ///
    /// Comment markers must already be stripped: this takes the statement itself, not the comment
    /// it was written in. Any text after the statement is kept as [`YAdr::tail`], and trailing
    /// `YYYY-MM-DD:` lines as [`YAdr::changes`], exactly as when scanning a file.
    ///
    /// ```
    /// use yadr::YAdr;
    ///
    /// let yadr = YAdr::parse(
    ///     "YADR: 2024-06-18 Store timestamps as UTC
    ///
    ///      In the context of comparing timestamps across machines, we faced ambiguity about
    ///      which offset each one was written in.
    ///
    ///      We decided for storing everything in UTC, and neglected recording a local offset
    ///      alongside each timestamp.
    ///
    ///      We did this to achieve unambiguous ordering, accepting an extra lookup when
    ///      rendering a timestamp in local time.
    ///
    ///      We think this is the right trade-off because ordering matters everywhere and
    ///      local-time rendering only in the user interface.",
    /// )?;
    ///
    /// assert_eq!(yadr.title, "2024-06-18 Store timestamps as UTC");
    /// assert_eq!(yadr.neglected_options, "recording a local offset\n     alongside each timestamp");
    /// # Ok::<_, miette::Report>(())
    /// ```
    ///
    /// Note the line break left in `neglected_options` above: the parser does not know how the
    /// text was wrapped, so call [`YAdr::tidy`] if you want each field on a single line.
    ///
    /// # Errors
    ///
    /// Returns a diagnostic pointing at the offending part of `statement` if it isn't a
    /// well-formed Y-Statement, or if it holds more than one.
    pub fn parse(statement: &'a str) -> miette::Result<YAdr<'a>> {
        let mut found = None;
        on_comment(1, statement, &mut |line, date, title, body| {
            if found.is_some() {
                miette::bail!(
                    "input holds more than one Y-Statement (found a second on line {line})"
                );
            }
            found = Some(parse_yadr_block(date, title, body)?);
            Ok(true)
        })?;
        found.ok_or_else(|| miette!("input does not hold a Y-Statement"))
    }
}

impl std::str::FromStr for YAdr<'static> {
    type Err = miette::Report;

    /// Parses a single Y-Statement, as [`YAdr::parse`] does.
    ///
    /// [`FromStr`](std::str::FromStr) cannot return a value borrowing from its input, so this
    /// always allocates. Prefer [`YAdr::parse`] where the input outlives the result.
    fn from_str(statement: &str) -> Result<Self, Self::Err> {
        YAdr::parse(statement).map(YAdr::into_owned)
    }
}

impl YAdr<'_> {
    /// Produces a `'static` version of the [`YAdr`].
    pub fn into_owned(self) -> YAdr<'static> {
        YAdr {
            date: self.date,
            title: Cow::Owned(self.title.into_owned()),
            in_context: Cow::Owned(self.in_context.into_owned()),
            facing_concern: Cow::Owned(self.facing_concern.into_owned()),
            chosen_option: Cow::Owned(self.chosen_option.into_owned()),
            neglected_options: Cow::Owned(self.neglected_options.into_owned()),
            to_achieve: Cow::Owned(self.to_achieve.into_owned()),
            accepted_downsides: Cow::Owned(self.accepted_downsides.into_owned()),
            because: Cow::Owned(self.because.into_owned()),
            tail: self.tail.map(|tail| Cow::Owned(tail.into_owned())),
            changes: self
                .changes
                .into_iter()
                .map(|(dt, change)| (dt, Cow::Owned(change.into_owned())))
                .collect(),
        }
    }

    /// Trims out extraneous whitespace that only appears as a result of comment formatting.
    ///
    /// For example, if a Y-Statement ADR comment contains (note the line break):
    ///
    /// ```rust
    /// # /*
    /// * We did this to achieve <system qualities/desired consequences>, accepting <downside
    /// * d/undesired consequences>.
    /// # */
    /// ```
    ///
    /// then [`YAdr::accepted_downsides`] will contain a newline character, which this method will
    /// get rid of.
    pub fn tidy(&mut self) {
        for field in [
            &mut self.in_context,
            &mut self.facing_concern,
            &mut self.chosen_option,
            &mut self.neglected_options,
            &mut self.to_achieve,
            &mut self.accepted_downsides,
            &mut self.because,
        ] {
            if field.trim().contains('\n') {
                let mut normal = String::with_capacity(field.len());
                // NOTE: split_whitespace() already ignores leading/trailing whitespace
                let mut words = field.split_whitespace().peekable();
                while let Some(word) = words.next() {
                    normal.push_str(word);
                    if words.peek().is_some() {
                        normal.push(' ');
                    }
                }
                *field = Cow::Owned(normal);
            }
        }
    }
}

impl fmt::Display for YAdr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "\
            YADR: {}\n\n\
            In the context of {}, \
            we faced {}.\n\n\
            We decided for {}, \
            and neglected {}.\n\n\
            We did this to achieve {}, \
            accepting {}.\n\n\
            We think this is the right trade-off because {}.",
            self.title,
            self.in_context,
            self.facing_concern,
            self.chosen_option,
            self.neglected_options,
            self.to_achieve,
            self.accepted_downsides,
            self.because
        )?;
        if let Some(tail) = &self.tail {
            write!(f, "\n\n{tail}")?;
        }
        if !self.changes.is_empty() {
            writeln!(f)?;
            for (dt, change) in &self.changes {
                write!(f, "\n{dt}: {change}")?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    /// Every language in `ALL` must be reachable by both of the string conversions, or a caller
    /// enumerating `ALL` would find entries it can't name or look up.
    #[test]
    fn language_conversions_round_trip() {
        for &language in Language::ALL {
            assert_eq!(
                Language::from_extension(language.extension()),
                Some(language),
                "{language} is not reachable from its own extension"
            );
            assert_eq!(
                Language::from_str(&language.to_string()).as_ref(),
                Ok(&language),
                "{language} is not reachable from its own name"
            );
        }
    }

    #[test]
    fn unknown_language_names_the_alternatives() {
        let e = Language::from_str("cobol").expect_err("cobol is not supported");
        assert_eq!(e.unrecognized(), "cobol");
        let msg = e.to_string();
        for language in Language::ALL {
            assert!(msg.contains(language.name()), "{msg:?} omits {language}");
        }
    }

    /// The whole point of `parse` over `FromStr` is that it can borrow out of its input.
    #[test]
    fn parse_borrows_from_its_input() {
        let statement = String::from(
            "YADR: 2024-06-18 a-title

            In the context of <u>, we faced <c>.

            We decided for <o>, and neglected <others>.

            We did this to achieve <q>, accepting <d>.

            We think this is the right trade-off because <r>.",
        );
        let yadr = YAdr::parse(&statement).expect("statement is well-formed");
        assert!(matches!(yadr.in_context, Cow::Borrowed("<u>")));
        assert_eq!(yadr.title, "2024-06-18 a-title");
        assert_eq!(yadr.date, Date::constant(2024, 6, 18));

        // ...whereas `FromStr` has to own, since it cannot name the input's lifetime.
        let owned = YAdr::from_str(&statement).expect("statement is well-formed");
        assert_eq!(owned, yadr.into_owned());
    }

    #[test]
    fn parse_rejects_a_second_statement() {
        let one = "YADR: 2024-06-18 a-title

            In the context of <u>, we faced <c>.

            We decided for <o>, and neglected <others>.

            We did this to achieve <q>, accepting <d>.

            We think this is the right trade-off because <r>.";
        let two = format!("{one}\n\n{}", one.replace("2024-06-18", "2024-06-19"));
        let e = YAdr::parse(&two).expect_err("two statements is not one statement");
        assert!(
            e.to_string().contains("more than one"),
            "unhelpful error: {e}"
        );
    }

    #[test]
    fn parse_rejects_input_without_a_statement() {
        let e = YAdr::parse("just some prose").expect_err("prose is not a Y-Statement");
        assert!(
            e.to_string().contains("does not hold a Y-Statement"),
            "unhelpful error: {e}"
        );
    }

    #[test]
    fn simple() {
        let mut called = false;
        find_yadr_sections_rs(
            r#"
                // YADR: 2023-11-28
                // hello world
            "#,
            |line, date, yt, yadr| {
                assert_eq!(line, 2);
                assert_eq!(date, Date::constant(2023, 11, 28));
                assert_eq!(yt, "2023-11-28");
                assert_eq!(yadr, "hello world");
                called = true;
                Ok(true)
            },
        )
        .unwrap();
        assert!(called);
    }

    #[test]
    fn no_trailing_newline() {
        let mut called = false;
        find_yadr_sections_rs(
            "// YADR: 2023-11-28\n// hello world",
            |line, date, yt, yadr| {
                assert_eq!(line, 1);
                assert_eq!(date, Date::constant(2023, 11, 28));
                assert_eq!(yt, "2023-11-28");
                assert_eq!(yadr, "hello world");
                called = true;
                Ok(true)
            },
        )
        .unwrap();
        assert!(called);
    }

    #[test]
    fn trailing_space() {
        let mut called = false;
        find_yadr_sections_rs(
            "// YADR: 2023-11-28\n// hello \n//world \n",
            |line, date, yt, yadr| {
                assert_eq!(line, 1);
                assert_eq!(date, Date::constant(2023, 11, 28));
                assert_eq!(yt, "2023-11-28");
                assert_eq!(yadr, "hello \nworld ");
                called = true;
                Ok(true)
            },
        )
        .unwrap();
        assert!(called);
    }

    #[test]
    fn consecutive_comments() {
        let mut called = false;
        find_yadr_sections_rs(
            r#"
                // YADR: 2023-11-28
                // hello
                // world
            "#,
            |line, date, yt, yadr| {
                assert_eq!(line, 2);
                assert_eq!(date, Date::constant(2023, 11, 28));
                assert_eq!(yt, "2023-11-28");
                assert_eq!(yadr, "hello\nworld");
                called = true;
                Ok(true)
            },
        )
        .unwrap();
        assert!(called);
    }

    #[test]
    fn multiline() {
        let mut called = false;
        find_yadr_sections_rs(
            r#"
                /* YADR: 2023-11-28
                hello world
                */
            "#,
            |line, date, yt, yadr| {
                assert_eq!(line, 2);
                assert_eq!(date, Date::constant(2023, 11, 28));
                assert_eq!(yt, "2023-11-28");
                assert_eq!(yadr, "hello world");
                called = true;
                Ok(true)
            },
        )
        .unwrap();
        assert!(called);
    }

    #[test]
    fn multiline_stars() {
        let mut called = false;
        find_yadr_sections_rs(
            r#"
                /*
                 * YADR: 2023-11-28
                 * hello world
                 */
            "#,
            |line, date, yt, yadr| {
                assert_eq!(line, 3);
                assert_eq!(date, Date::constant(2023, 11, 28));
                assert_eq!(yt, "2023-11-28");
                assert_eq!(yadr, "hello world");
                called = true;
                Ok(true)
            },
        )
        .unwrap();
        assert!(called);
    }

    #[test]
    fn multiline_leading() {
        let mut called = false;
        find_yadr_sections_rs(
            r#"
                /*
                YADR: 2023-11-28
                hello world
                */
            "#,
            |line, date, yt, yadr| {
                assert_eq!(line, 3);
                assert_eq!(date, Date::constant(2023, 11, 28));
                assert_eq!(yt, "2023-11-28");
                assert_eq!(yadr, "hello world");
                called = true;
                Ok(true)
            },
        )
        .unwrap();
        assert!(called);
    }

    #[test]
    fn multiline_multi_line() {
        let mut called = false;
        find_yadr_sections_rs(
            r#"
                /*
                YADR: 2023-11-28
                hello
                world
                */
            "#,
            |line, date, yt, yadr| {
                assert_eq!(line, 3);
                assert_eq!(date, Date::constant(2023, 11, 28));
                assert_eq!(yt, "2023-11-28");
                assert_eq!(yadr, "hello\nworld");
                called = true;
                Ok(true)
            },
        )
        .unwrap();
        assert!(called);
    }

    #[test]
    fn multiline_trailing_space() {
        let mut called = false;
        find_yadr_sections_rs(
            "/* YADR: 2023-11-28\nhello \nworld */",
            |line, date, yt, yadr| {
                assert_eq!(line, 1);
                assert_eq!(date, Date::constant(2023, 11, 28));
                assert_eq!(yt, "2023-11-28");
                assert_eq!(yadr, "hello \nworld ");
                called = true;
                Ok(true)
            },
        )
        .unwrap();
        assert!(called);
    }

    #[test]
    fn full_yadr() {
        let source = "YADR: 2023-11-28 some-title

        In the context of <use case/user story u>, we faced <concern c>.

        We decided for <option o>, and neglected <other options>.

        We did this to achieve <system qualities/desired consequences>, accepting
        <downside d/undesired consequences>.

        We think this is the right trade-off because <additional rationale>.
        ";
        let mut yadr = YAdr::parse(source).unwrap();
        yadr.tidy();
        assert_eq!(yadr.date, Date::constant(2023, 11, 28));
        assert_eq!(yadr.title, "2023-11-28 some-title");
        assert_eq!(yadr.in_context, "<use case/user story u>");
        assert_eq!(yadr.facing_concern, "<concern c>");
        assert_eq!(yadr.chosen_option, "<option o>");
        assert_eq!(yadr.neglected_options, "<other options>");
        assert_eq!(yadr.to_achieve, "<system qualities/desired consequences>");
        assert_eq!(
            yadr.accepted_downsides,
            "<downside d/undesired consequences>"
        );
        assert_eq!(yadr.because, "<additional rationale>");
        assert_eq!(yadr.tail, None);

        // `Display` is meant to emit the canonical format, which means what it prints has to be
        // something we can read back. This is the cheapest way to keep the two in step.
        assert_eq!(
            YAdr::parse(&yadr.to_string()).expect("Display emits the canonical format"),
            yadr
        );
    }

    #[test]
    fn parse_one() {
        let mut yadrs = Vec::new();
        find_all(
            r#"
                /*
                 * YADR: 2023-11-28 foo-bar also
                 * In the context of <use case/user story u>, we faced <concern c>.
                 *
                 * We decided for <option o>, and neglected <other options>.
                 *
                 * We did this to achieve <system qualities/desired consequences>, accepting
                 * <downside d/undesired consequences>.
                 *
                 * We think this is the right trade-off because <additional rationale>.
                 */
            "#,
            Language::Rust,
            |line, yadr| {
                yadrs.push((line, yadr.into_owned()));
                Ok(true)
            },
        )
        .unwrap();
        assert_eq!(yadrs.len(), 1);
        assert_eq!(yadrs[0].0, 3);
        assert_eq!(yadrs[0].1.date, Date::constant(2023, 11, 28));
        assert_eq!(yadrs[0].1.title, "2023-11-28 foo-bar also");
        assert_eq!(yadrs[0].1.in_context, "<use case/user story u>");
        assert_eq!(yadrs[0].1.facing_concern, "<concern c>");
        assert_eq!(yadrs[0].1.chosen_option, "<option o>");
        assert_eq!(yadrs[0].1.neglected_options, "<other options>");
        assert_eq!(
            yadrs[0].1.to_achieve,
            "<system qualities/desired consequences>"
        );
        assert_eq!(
            yadrs[0].1.accepted_downsides,
            "<downside d/undesired consequences>"
        );
        assert_eq!(yadrs[0].1.because, "<additional rationale>");
        assert_eq!(yadrs[0].1.tail, None);
    }

    #[test]
    fn parse_one_with_tail() {
        let mut yadrs = Vec::new();
        find_all(
            r#"
                /*
                 * YADR: 2023-11-28 foo-bar also
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
            "#,
            Language::Rust,
            |line, yadr| {
                yadrs.push((line, yadr.into_owned()));
                Ok(true)
            },
        )
        .unwrap();
        assert_eq!(yadrs.len(), 1);
        assert_eq!(yadrs[0].0, 3);
        assert_eq!(yadrs[0].1.date, Date::constant(2023, 11, 28));
        assert_eq!(yadrs[0].1.title, "2023-11-28 foo-bar also");
        assert_eq!(yadrs[0].1.in_context, "<use case/user story u>");
        assert_eq!(yadrs[0].1.facing_concern, "<concern c>");
        assert_eq!(yadrs[0].1.chosen_option, "<option o>");
        assert_eq!(yadrs[0].1.neglected_options, "<other options>");
        assert_eq!(
            yadrs[0].1.to_achieve,
            "<system qualities/desired consequences>"
        );
        assert_eq!(
            yadrs[0].1.accepted_downsides,
            "<downside d/undesired consequences>"
        );
        assert_eq!(yadrs[0].1.because, "<additional rationale>");
        assert_eq!(
            yadrs[0].1.tail.as_ref().unwrap(),
            "Also, foobar to the baz."
        );
    }

    #[test]
    fn parse_one_with_changes() {
        let mut yadrs = Vec::new();
        find_all(
            r#"
                /*
                 * YADR: 2023-11-28 foo-bar also
                 * In the context of <use case/user story u>, we faced <concern c>.
                 *
                 * We decided for <option o>, and neglected <other options>.
                 *
                 * We did this to achieve <system qualities/desired consequences>, accepting
                 * <downside d/undesired consequences>.
                 *
                 * We think this is the right trade-off because <additional rationale>.
                 *
                 * 2023-11-29: jon did a thing
                 * 2023-11-30: james did another thing
                 */
            "#,
            Language::Rust,
            |line, yadr| {
                yadrs.push((line, yadr.into_owned()));
                Ok(true)
            },
        )
        .unwrap();
        assert_eq!(yadrs.len(), 1);
        assert_eq!(yadrs[0].1.tail, None);
        assert_eq!(yadrs[0].1.changes.len(), 2);
        assert_eq!(yadrs[0].1.changes[0].0, Date::constant(2023, 11, 29));
        assert_eq!(yadrs[0].1.changes[0].1, "jon did a thing");
        assert_eq!(yadrs[0].1.changes[1].0, Date::constant(2023, 11, 30));
        assert_eq!(yadrs[0].1.changes[1].1, "james did another thing");
    }

    #[test]
    fn parse_one_with_changes_and_tail() {
        let mut yadrs = Vec::new();
        find_all(
            r#"
                /*
                 * YADR: 2023-11-28 foo-bar also
                 * In the context of <use case/user story u>, we faced <concern c>.
                 *
                 * We decided for <option o>, and neglected <other options>.
                 *
                 * We did this to achieve <system qualities/desired consequences>, accepting
                 * <downside d/undesired consequences>.
                 *
                 * We think this is the right trade-off because <additional rationale>.
                 *
                 * And also there's a tail, followed by changes:
                 *
                 * 2023-11-29: jon did a thing
                 * 2023-11-30: james did another thing
                 */
            "#,
            Language::Rust,
            |line, yadr| {
                yadrs.push((line, yadr.into_owned()));
                Ok(true)
            },
        )
        .unwrap();
        assert_eq!(yadrs.len(), 1);
        assert_eq!(
            yadrs[0].1.tail.as_ref().unwrap(),
            "And also there's a tail, followed by changes:"
        );
        assert_eq!(yadrs[0].1.changes.len(), 2);
        assert_eq!(yadrs[0].1.changes[0].0, Date::constant(2023, 11, 29));
        assert_eq!(yadrs[0].1.changes[0].1, "jon did a thing");
        assert_eq!(yadrs[0].1.changes[1].0, Date::constant(2023, 11, 30));
        assert_eq!(yadrs[0].1.changes[1].1, "james did another thing");
    }

    #[test]
    fn parse_two() {
        let mut yadrs = Vec::new();
        find_all(
            r#"
                /*
                 * YADR: 2023-11-28 foo-bar also
                 * In the context of <use case/user story u>, we faced <concern c>.
                 *
                 * We decided for <option o>, and neglected <other options>.
                 *
                 * We did this to achieve <system qualities/desired consequences>, accepting
                 * <downside d/undesired consequences>.
                 *
                 * We think this is the right trade-off because <additional rationale>.
                 */

                // YADR: 2023-11-29 bar-baz
                // In the context of <use case/user story u>, we faced <concern c>.
                //
                // We decided for <option o>, and neglected <other options>.
                //
                // We did this to achieve <system qualities/desired consequences>, accepting
                // <downside d/undesired consequences>.
                //
                // We think this is the right trade-off because <additional rationale>.
            "#,
            Language::Rust,
            |line, yadr| {
                yadrs.push((line, yadr.into_owned()));
                Ok(true)
            },
        )
        .unwrap();
        assert_eq!(yadrs.len(), 2);
        assert_eq!(yadrs[0].0, 3);
        assert_eq!(yadrs[1].0, 14);
        assert_eq!(yadrs[0].1.date, Date::constant(2023, 11, 28));
        assert_eq!(yadrs[0].1.title, "2023-11-28 foo-bar also");
        assert_eq!(yadrs[1].1.date, Date::constant(2023, 11, 29));
        assert_eq!(yadrs[1].1.title, "2023-11-29 bar-baz");
        for (_, yadr) in &yadrs {
            assert_eq!(yadr.in_context, "<use case/user story u>");
            assert_eq!(yadr.facing_concern, "<concern c>");
            assert_eq!(yadr.chosen_option, "<option o>");
            assert_eq!(yadr.neglected_options, "<other options>");
            assert_eq!(yadr.to_achieve, "<system qualities/desired consequences>");
            assert_eq!(
                yadr.accepted_downsides,
                "<downside d/undesired consequences>"
            );
            assert_eq!(yadr.because, "<additional rationale>");
            assert_eq!(yadr.tail, None);
        }
    }

    #[test]
    fn parse_two_in_one() {
        let mut yadrs = Vec::new();
        find_all(
            r#"
                /*
                 * YADR: 2023-11-28 foo-bar also
                 * In the context of <use case/user story u>, we faced <concern c>.
                 *
                 * We decided for <option o>, and neglected <other options>.
                 *
                 * We did this to achieve <system qualities/desired consequences>, accepting
                 * <downside d/undesired consequences>.
                 *
                 * We think this is the right trade-off because <additional rationale>.

                 YADR: 2023-11-29 bar-baz
                 In the context of <use case/user story u>, we faced <concern c>.

                 We decided for <option o>, and neglected <other options>.

                 We did this to achieve <system qualities/desired consequences>, accepting
                 <downside d/undesired consequences>.

                 We think this is the right trade-off because <additional rationale>.
                 */
            "#,
            Language::Rust,
            |line, yadr| {
                yadrs.push((line, yadr.into_owned()));
                Ok(true)
            },
        )
        .unwrap();
        assert_eq!(yadrs.len(), 2);
        assert_eq!(yadrs[0].0, 3);
        assert_eq!(yadrs[1].0, 13);
        assert_eq!(yadrs[0].1.date, Date::constant(2023, 11, 28));
        assert_eq!(yadrs[0].1.title, "2023-11-28 foo-bar also");
        assert_eq!(yadrs[1].1.date, Date::constant(2023, 11, 29));
        assert_eq!(yadrs[1].1.title, "2023-11-29 bar-baz");
        for (_, yadr) in &yadrs {
            assert_eq!(yadr.in_context, "<use case/user story u>");
            assert_eq!(yadr.facing_concern, "<concern c>");
            assert_eq!(yadr.chosen_option, "<option o>");
            assert_eq!(yadr.neglected_options, "<other options>");
            assert_eq!(yadr.to_achieve, "<system qualities/desired consequences>");
            assert_eq!(
                yadr.accepted_downsides,
                "<downside d/undesired consequences>"
            );
            assert_eq!(yadr.because, "<additional rationale>");
            assert_eq!(yadr.tail, None);
        }
    }

    /// Parses a Y-Statement taken verbatim from a real code base (the [`dson`] crate) rather than
    /// one written to fit the parser, so that we notice if the format only works for prose we
    /// wrote ourselves.
    ///
    /// [`dson`]: https://github.com/helsing-ai/dson/blob/607ee3a8a7e9a56cb80746b66faa9e6283b5b276/src/crdts/orarray.rs#L434-L454
    #[test]
    fn realistic() {
        let mut yadrs = Vec::new();
        find_all(
            r#"
                /*
                 * YADR: 2024-06-18 Array elements without a position defined
                 * In the context of dealing with array elements which don't have a position
                 * defined, we faced a decision of how to expose these elements through the public
                 * OrArray API.
                 *
                 * We decided for assigning these elements an arbitrary but deterministic position,
                 * as a function of their uid, and neglected to attempt to map them to the start or
                 * end of the array, or provide a separate API for access to position-less
                 * elements, when their uid is not yet known.
                 *
                 * We did this to achieve minimal impact to the user-facing API, to avoid increasing
                 * the cognitive burden of using this crate, and to ensure that every node has a
                 * consistent view of the array when they share the same state, accepting that users
                 * may be surprised to find that a non-move operation (like a delete) can result in
                 * an element being assigned a different position.
                 *
                 * We think this is the right trade-off because this is a rare edge case, and
                 * placing the burden of handling it on users (by providing a separate access
                 * interface) would've been unreasonable. Additionally, non-deterministic views of
                 * the array would have violated a core assumption that nodes in sync with each
                 * other have the same view of the state.
                 */
            "#,
            Language::Rust,
            |line, mut yadr| {
                yadr.tidy();
                yadrs.push((line, yadr.into_owned()));
                Ok(true)
            },
        )
        .unwrap();
        assert_eq!(yadrs.len(), 1);
        assert_eq!(yadrs[0].0, 3);
        assert_eq!(yadrs[0].1.date, Date::constant(2024, 6, 18));
        assert_eq!(
            yadrs[0].1.title,
            "2024-06-18 Array elements without a position defined"
        );
        assert_eq!(
            yadrs[0].1.in_context,
            "dealing with array elements which don't have a position defined"
        );
        assert_eq!(
            yadrs[0].1.facing_concern,
            "a decision of how to expose these elements through the public OrArray API"
        );
        assert_eq!(
            yadrs[0].1.chosen_option,
            "assigning these elements an arbitrary but deterministic position, as a function of their uid"
        );
        assert_eq!(yadrs[0].1.neglected_options, "to attempt to map them to the start or end of the array, or provide a separate API for access to position-less elements, when their uid is not yet known");
        assert_eq!(yadrs[0].1.to_achieve, "minimal impact to the user-facing API, to avoid increasing the cognitive burden of using this crate, and to ensure that every node has a consistent view of the array when they share the same state");
        assert_eq!(yadrs[0].1.accepted_downsides, "that users may be surprised to find that a non-move operation (like a delete) can result in an element being assigned a different position");
        assert_eq!(yadrs[0].1.because, "this is a rare edge case, and placing the burden of handling it on users (by providing a separate access interface) would've been unreasonable. Additionally, non-deterministic views of the array would have violated a core assumption that nodes in sync with each other have the same view of the state");
        assert_eq!(yadrs[0].1.tail, None);
    }
}
