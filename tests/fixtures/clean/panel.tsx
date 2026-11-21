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
