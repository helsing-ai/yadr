// A fixture for yadr's end-to-end tests. Never executed; only ever read as text.
//
// The statement sits inside the markup, in a single `{/* ... */}` container. That is the only
// comment syntax JSX offers in a children position, so a decision effected inside a render tree
// has nowhere else to be recorded next to the code it governs.
//
// It has to be one container rather than a run of short ones: each container wraps its comment in
// a `jsx_expression` of its own, which makes the comments only-children rather than siblings, and
// only siblings are grouped into a single block. `panel.tsx` covers the ordinary block comment
// above a declaration.

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
