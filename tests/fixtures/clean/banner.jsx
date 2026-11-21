export function Banner({ message }) {
  return (
    <aside role="status">
      {/*
       * YADR: 1970-01-03 <title>
       *
       * In the context of <in_context>, we faced <facing_concern>.
       *
       * We decided for <chosen_option>, and neglected <neglected_options>.
       *
       * We did this to achieve <to_achieve>, accepting <accepted_downsides>.
       *
       * We think this is the right trade-off because <because>.
       */}
      {message}
      {/* A lone JSX comment, which holds no statement and so contributes none. */}
    </aside>
  );
}
