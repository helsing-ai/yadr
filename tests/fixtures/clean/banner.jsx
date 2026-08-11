// A fixture for yadr's end-to-end tests. Never executed; only ever read as text.
//
// Two things here that no other fixture covers: the body is JSX, which plain JavaScript cannot
// parse, and the markup holds a `{/* ... */}` comment that is not a Y-Statement. A JSX comment is
// a comment node like any other, so one sitting a few lines from a real statement must neither be
// read as part of it nor mistaken for one of its own.

/*
 * YADR: 2024-09-05 Render the banner as a sibling of the page content
 *
 * In the context of a status banner that has to be announced as soon as it appears, we faced
 * the question of where to mount it in the tree.
 *
 * We decided for rendering it as a sibling of the page content, and neglected mounting it into
 * a portal at the document root.
 *
 * We did this to achieve an announcement that fires without the banner escaping the layout it
 * belongs to, accepting that a page scrolling its own content has to leave room for it.
 *
 * We think this is the right trade-off because the layout is shared by every page, whereas a
 * portal would leave the banner's position depending on styles it cannot see.
 */
export function Banner({ message }) {
  return (
    <aside role="status">
      {/* Deliberately not a Y-Statement. */}
      {message}
    </aside>
  );
}
