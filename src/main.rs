// allow comment-only Rust code blocks
#![allow(rustdoc::invalid_rust_codeblocks)]

use camino::Utf8PathBuf as PathBuf;
use clap::{Parser, Subcommand};
use ignore::{overrides::OverrideBuilder, WalkBuilder};
use miette::{IntoDiagnostic, WrapErr};
use std::io::{self, BufWriter, Write};
use yadr::{Language, YAdr};

/// Access Y-Statement ADRs in source code comments.
///
/// These ADRs take the form:
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
///
/// and should be used to *concisely* convey important architectural decisions in software projects.
///
/// For more information about the what and why, see
///
///     <https://adr.github.io/>
///
/// and
///
///     <https://medium.com/olzzio/y-statements-10eb07b5a177>
///
/// Here is one taken from the dson crate:
///
///     <https://github.com/helsing-ai/dson/blob/607ee3a8a7e9a56cb80746b66faa9e6283b5b276/src/crdts/orarray.rs#L434-L454>
///
/// ```rust
/// /*
///  * YADR: 2024-06-18 Array elements without a position defined
///  *
///  * In the context of dealing with array elements which don't have a position defined,
///  * we faced a decision of how to expose these elements through the public OrArray API.
///  *
///  * We decided for assigning these elements an arbitrary but deterministic position, as
///  * a function of their uid, and neglected to attempt to map them to the start or end of
///  * the array, or provide a separate API for access to position-less elements, when their
///  * uid is not yet known.
///  *
///  * We did this to achieve minimal impact to the user-facing API, to avoid increasing the
///  * cognitive burden of using this crate, and to ensure that every node has a consistent
///  * view of the array when they share the same state, accepting that users may be
///  * surprised to find that a non-move operation (like a delete) can result in an element
///  * being assigned a different position.
///  *
///  * We think this is the right trade-off because this is a rare edge case, and placing
///  * the burden of handling it on users (by providing a separate access interface)
///  * would've been unreasonable. Additionally, non-deterministic views of the array would
///  * have violated a core assumption that nodes in sync with each other have the same view
///  * of the state.
///  */
/// ```
///
/// Note that we diverge slightly from the original Y-Statement format to make the comment easier
/// to digest!
#[derive(Parser, Debug)]
#[command(author, version, verbatim_doc_comment)]
struct Args {
    #[command(subcommand)]
    mode: Mode,
}

/// Which files a subcommand should search.
///
/// Every subcommand takes these, so that the path reads where a reader would expect it to
/// (`yadr ls src/`). They're repeated into each subcommand rather than hoisted to [`Args`]
/// because clap only supports `global = true` for options, not for positional arguments, so a
/// top-level `root` would have to be written *before* the subcommand (`yadr src/ ls`).
#[derive(clap::Args, Debug)]
struct Scope {
    /// Exclude paths from search (supports globbing)
    #[arg(long)]
    exclude: Vec<String>,

    /// Path to search for Y-statements in.
    #[arg(default_value_t = PathBuf::from("."))]
    root: PathBuf,
}

#[derive(Subcommand, Debug)]
enum Mode {
    /// List all found Y-Statements.
    #[clap(alias = "ls")]
    List {
        /// List the file names that contain Y-Statements.
        ///
        /// That is, don't list all the Y-Statements themselves.
        #[clap(short = 'L')]
        only_files: bool,

        #[command(flatten)]
        scope: Scope,
    },

    /// Run in check-only mode.
    ///
    /// Exits non-zero if any Y-Statement fails to parse. Prints nothing otherwise.
    Check {
        #[command(flatten)]
        scope: Scope,
    },

    /// Show the full Y-Statement for a given ADR.
    Show {
        /// Show only Y-Statements whose title starts with this, such as a date.
        id: String,

        #[command(flatten)]
        scope: Scope,
    },
}

impl Mode {
    fn scope(&self) -> &Scope {
        match self {
            Mode::List { scope, .. } | Mode::Check { scope } | Mode::Show { scope, .. } => scope,
        }
    }
}

fn main() -> miette::Result<()> {
    let args = Args::parse();
    let scope = args.mode.scope();

    let mut walk = WalkBuilder::new(&scope.root);
    walk.hidden(false);
    let mut excludes = OverrideBuilder::new(".");
    for exclude in &scope.exclude {
        excludes
            .add(&format!("!{exclude}"))
            .into_diagnostic()
            .wrap_err_with(|| format!("bad exclude rule {exclude}"))?;
    }
    walk.overrides(
        excludes
            .build()
            .into_diagnostic()
            .wrap_err("assemble exclude list")?,
    );

    // Output goes through an explicit locked, buffered handle rather than `println!` for two
    // reasons: `println!` locks and flushes on every call, which is needless work when listing a
    // large tree; and it *panics* when the reader goes away, which happens routinely for a tool
    // like this (`yadr ls | head`). Writing by hand lets us treat a closed pipe as the ordinary
    // end of the run instead. `BrokenPipe` doesn't exist on Windows, but `WriteZero`/`Other` from
    // a closed handle are handled the same way by `stop_on_broken_pipe`.
    let stdout = std::io::stdout();
    let mut out = BufWriter::new(stdout.lock());

    'walk: for entry in walk.build() {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();
        if !path.is_file() {
            // This skips any directories that are named similar to our expected files.
            // While this usually does not happen in day-to-day code bases this
            // can happen in Nixpkgs or similar complicated setups.
            continue;
        }

        // Anything we don't have a parser for is simply not our business; a source tree is mostly
        // made of such files.
        let Some(language) = path
            .extension()
            .and_then(|ext| ext.to_str())
            .and_then(Language::from_extension)
        else {
            continue;
        };

        let source = std::fs::read_to_string(path)
            .into_diagnostic()
            .wrap_err_with(|| format!("read {}", path.display()))?;

        let mut printed_file_title = false;
        let mut pipe_closed = false;
        yadr::find_all(&source, language, |line: usize, mut yadr: YAdr<'_>| {
            if let Mode::Show { id, .. } = &args.mode {
                if !yadr.title.starts_with(id) {
                    return Ok(true);
                }
            }

            let wrote = match &args.mode {
                Mode::Check { .. } => Ok(()),
                Mode::Show { .. } => {
                    yadr.tidy();
                    writeln!(out, "# {}", yadr.title)
                        .and_then(|()| writeln!(out, "## from {}:{line}", path.display()))
                        .and_then(|()| writeln!(out, "{yadr}"))
                }
                Mode::List { only_files, .. } => {
                    if *only_files {
                        // one line per file, so there's nothing more to learn from this file
                        return stop_on_broken_pipe(
                            writeln!(out, "{}", path.display()),
                            &mut pipe_closed,
                        )
                        .map(|_| false);
                    }
                    if !printed_file_title {
                        printed_file_title = true;
                        if let Err(e) = writeln!(out, "==> {}", path.display()) {
                            return stop_on_broken_pipe(Err(e), &mut pipe_closed);
                        }
                    }
                    if let Some((last_changed, _)) = yadr.changes.last() {
                        writeln!(out, " -> {} (last changed: {})", yadr.title, last_changed)
                    } else {
                        writeln!(out, " -> {}", yadr.title)
                    }
                }
            };
            stop_on_broken_pipe(wrote, &mut pipe_closed)
        })
        .with_context(|| format!("in {}", path.display()))?;

        if pipe_closed {
            break 'walk;
        }
    }

    // An error here is the same closed-pipe case as above, just noticed at flush time.
    match out.flush() {
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        other => other.into_diagnostic().wrap_err("flush stdout"),
    }
}

/// Converts a write result into "keep going?", treating a closed reader as a reason to stop.
///
/// Sets `pipe_closed` so the caller can also abandon the surrounding directory walk; without it
/// we'd keep parsing every remaining file just to throw the output away.
fn stop_on_broken_pipe(wrote: io::Result<()>, pipe_closed: &mut bool) -> miette::Result<bool> {
    match wrote {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => {
            *pipe_closed = true;
            Ok(false)
        }
        Err(e) => Err(e).into_diagnostic().wrap_err("write to stdout"),
    }
}
