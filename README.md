# yadr

Command-line tool for accessing Y-Statement-style ADRs in code comments.

See also [Helsing's announcement blog post][blog].

[blog]: https://blog.helsing.ai/posts/on-comments/

## Description

ADRs, or [Architecture Decision Records][ADRs] are justifications for
software design choices that address a functional or non-functional
requirement that is architecturally significant. These take many forms,
with [Markdown ADRs][MADR] being particularly common. However, those
are removed from the code that they document, which makes them harder to
remember to look for and harder to keep up to date.

There exists a shorter version of an ADR, known as a
[Y-statement][ystmt]. Y-statements take the form

> In the context of {use case/user story u}, facing {concern c} we
> decided for {option o} and neglected {other options}, to achieve
> {system qualities/desired consequences}, accepting {downside
> d/undesired consequences}, because {additional rationale}.

and aim to very concisely capture just the essential bits of a taken
decision. It's beneficial to maintain a changelog at the bottom of the
Y-statement to keep a historical reference of the decision change and 
track major changes in the decision.

These are short enough that they can be placed directly in comments in
source code near where a given decision is actually effected. By placing
the decision record in the code, we make it more likely that they are
seen, read, and updated when someone is in the related block of code.

The downside to placing these records in source code is that they are
harder to spot from the outside. Sure, you'll see them when browsing
that part of the code, but what if you want to survey the various
architectural decisions made in a project overall?

Enter this tool.

This tool expects you to write Y-Statement ADRs in comments in the
following format:

```rust
/*
 * YADR: YYYY-MM-DD some title
 *
 * In the context of {use case/user story u}, we faced {concern c}.
 *
 * We decided for {option o}, and neglected {other options}.
 *
 * We did this to achieve {system qualities/desired consequences},
 * accepting {downside/undesired consequences}.
 *
 * We think this is the right trade-off because {additional rationale}.
 *
 * {Optional changelog}
 * YYYY-MM-DD:  <description of change>
 */
```

(note that this is a modified Y-statement that's easier to digest!)

With the Y-Statements written this way, `yadr` can survey a whole source
tree for you, print any one of them in full, and check in CI that the
ones you have still follow the format.

For example, here is one taken from the [`dson`] crate:

```rust
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
```

### Y-Statements in JSX and TSX

In `.jsx` and `.tsx` files, a statement placed inside JSX markup has to be written in a **single** `{/* ... */}` container:

```jsx
<aside role="status">
  {/*
   * YADR: YYYY-MM-DD some title
   *
   * In the context of {use case/user story u}, we faced {concern c}.
   *
   * ...
   */}
</aside>
```

[ADRs]: https://adr.github.io/
[MADR]: https://adr.github.io/madr/
[ystmt]: https://medium.com/olzzio/y-statements-10eb07b5a177
[`dson`]: https://github.com/helsing-ai/dson/blob/607ee3a8a7e9a56cb80746b66faa9e6283b5b276/src/crdts/orarray.rs#L434-L454

## Installation

```console
$ cargo install yadr
```

Or, from a checkout of this repository:

```console
$ cargo install --path .
```

## Usage

`yadr` walks a directory tree, parses every file whose extension it
recognises, and reports on the Y-Statements it finds. It understands
Rust (`.rs`), Python (`.py`), and Nix (`.nix`), and it honours
`.gitignore`.

Every subcommand takes an optional path to search — the current
directory by default — and any number of `--exclude` globs.

### `yadr ls` — survey the decisions in a tree

```console
$ yadr ls src/
==> src/crdts.rs
 -> 2024-05-06 Removal of `.alive` tracking
==> src/causal_context.rs
 -> 2024-04-19 Allocation of identifier bits
```

A statement that carries a changelog also reports when it was last
revisited, which is often the first thing you want to know about a
decision:

```console
 -> 2024-01-15 Store timestamps as UTC (last changed: 2024-02-01)
```

Pass `-L` to print only the names of files that contain Y-Statements.

### `yadr show` — read one in full

Give it any prefix of the title, which in practice means the date:

```console
$ yadr show 2024-05-06
```

The statement is printed with each paragraph reflowed onto a single
line, and prefixed with the file and line it came from, so you can go
and read the code it belongs to.

### `yadr check` — verify that every statement parses

```console
$ yadr check
```

Prints nothing and exits zero if every Y-Statement in the tree follows
the format. Otherwise it exits non-zero and points at what is wrong:

```console
Error:   × in src/storage.rs
  ├─▶ in comment on line 7
  ├─▶ in Y-Statement starting on line 8
  ├─▶ in paragraph 2 of that statement
  ╰─▶ Y-Statement did not match expected format
   ╭─[5:1]
 4 │
 5 │ We chose one of the options available to us, and forgot the rest of the format.
   · ──────────────────────────────────────┬─────────────────────────────────────────
   ·                                       ╰── expected "We decided for <..>, and neglected <..>."
   ╰────
  help: Note that punctuation is significant; you have to match every , and .
```

The format is strict on purpose. A Y-Statement is only worth writing if
it makes you answer all of the questions, and it is only worth reading
if you can trust that it did.

## Using yadr as a library

The crate that backs the binary is published too, if you want to do
something with Y-Statements that the CLI doesn't do — render them to a
web page, say, or pull them out of a file format `yadr` can't read.

```rust
use yadr::{find_all, Language, YAdr};

let source = std::fs::read_to_string("src/lib.rs")?;
find_all(&source, Language::Rust, |line, yadr| {
    println!("{}:{line} {}", "src/lib.rs", yadr.title);
    Ok(true)
})?;
```

Use `YAdr::parse` if you already have a single statement in hand rather
than a whole file. See [the API documentation][docs] for the rest.

[docs]: https://docs.rs/yadr

## Frequently Asked Questions

**1. How should the date be formatted on the first line of a YADR,**
**and what is the recommended method for documenting changes?**

The date on a YADR should typically reflect the "as of" date—indicating
when the decision documented in the YADR is applicable.
This date should be updated whenever the YADR is significantly changed.

Here are the recommended methods for documenting changes:
1. **Minor Edits**: If you make small fixes to the text that don't alter the
content or decision's validity, simply make the necessary corrections without
updating the date or maintaining a changelog.

2. **Content Clarifications or Additions**: If you change or add reasoning,
include missing information, or rewrite sections for clarity, add an entry to
the changelog noting the modification.

3. **Decision Changes**: If the change involves updating the decision itself,
consider rewriting the YADR, including the title and the date, to clearly
reflect the new decision. In this case, you must keep a single line in the
changelog indicating that a significant decision change has occurred, including
the old title (and date) of the previous YADR.

These practices ensure that YADRs provide a clear historical record of the
decision-making process while maintaining their relevance and accuracy over
time.

## Project status

`yadr` does what it says on the tin, and is stable in that. It finds and
verifies Y-Statements in `.rs`, `.py`, and `.nix` files.

Things we would like to exist, and would welcome help with:

- Support for more file formats (`.toml`, `.yaml`, `.json`, `.md`,
  `.ts`, and so on).
- Following the source links in comments, which `miette` already
  supports.
- `yadr ls` reporting every error it finds, rather than stopping at the
  first.

## Contributing

```console
$ git clone https://github.com/helsing-ai/yadr && cd yadr
$ cargo test
```

The test suite is in three parts: unit tests in `src/lib.rs` for the
parser, snapshot tests in `tests/instas.rs` covering what each kind of
input parses to and what each kind of malformed input reports, and
end-to-end tests in `tests/cli.rs` that run the built binary over the
fixture trees in `tests/fixtures/`.

The snapshot tests use [`insta`](https://insta.rs/). If you change what
the parser produces, run `cargo insta test --review` and read every diff
before accepting it — a snapshot that quietly absorbs a bug is worse
than no snapshot at all.

## License

Licensed under either of

* Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
* MIT license
   ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
