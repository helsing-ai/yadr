// A fixture for yadr's end-to-end tests. Never executed or type-checked; only ever read as text.
//
// The body carries both JSX and a type annotation, so the file is only parseable by the TSX
// grammar.

/**
 * YADR: 1970-01-04 <title>
 *
 * In the context of <in_context>, we faced <facing_concern>.
 *
 * We decided for <chosen_option>, and neglected <neglected_options>.
 *
 * We did this to achieve <to_achieve>, accepting <accepted_downsides>.
 *
 * We think this is the right trade-off because <because>.
 */
export function Panel({
  title,
  children,
}: {
  title: string;
  children: unknown;
}) {
  return <section aria-label={title}>{children}</section>;
}
