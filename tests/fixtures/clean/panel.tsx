// A fixture for yadr's end-to-end tests. Never executed or type-checked; only ever read as text.
//
// The body carries both JSX and a type annotation, so the file is only parseable by the TSX
// grammar: the plain TypeScript grammar rejects the markup, and a JavaScript grammar rejects the
// annotation. That is what this fixture is here to pin down, rather than any comment form.

/**
 * YADR: 2024-08-14 Take the panel's contents as children
 *
 * In the context of a panel that different pages need to fill with different content, we
 * faced the question of how callers should supply that content.
 *
 * We decided for taking the contents as children, and neglected a render prop for each region
 * of the panel.
 *
 * We did this to achieve markup at the call site that reads in the order it renders, accepting
 * that the panel cannot reposition what it is handed.
 *
 * We think this is the right trade-off because every caller today lays its content out top to
 * bottom, and a render prop can be added later without disturbing the ones that never wanted
 * it.
 */
export function Panel({ title, children }: { title: string; children: unknown }) {
  return <section aria-label={title}>{children}</section>;
}
