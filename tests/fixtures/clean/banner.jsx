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
       * YADR: 2024-09-05 Render the banner as a sibling of the page content
       *
       * In the context of a status banner that has to be announced as soon as it appears, we
       * faced the question of where to mount it in the tree.
       *
       * We decided for rendering it as a sibling of the page content, and neglected mounting
       * it into a portal at the document root.
       *
       * We did this to achieve an announcement that fires without the banner escaping the
       * layout it belongs to, accepting that a page scrolling its own content has to leave
       * room for it.
       *
       * We think this is the right trade-off because the layout is shared by every page,
       * whereas a portal would leave the banner's position depending on styles it cannot see.
       */}
      {message}
      {/* A lone JSX comment, which holds no statement and so contributes none. */}
    </aside>
  );
}
