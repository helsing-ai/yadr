// A fixture for yadr's end-to-end tests. Never executed or type-checked; only ever read as text.
// The statement is written as a JSDoc block, which is the shape TypeScript code bases reach for
// and the one no other fixture in this tree covers.

/**
 * YADR: 2024-06-11 Send the auth token in a header
 *
 * In the context of calling an API that accepts credentials in more than one place, we faced
 * the question of where to put the token.
 *
 * We decided for sending it in an Authorization header, and neglected passing it as a query
 * parameter.
 *
 * We did this to achieve credentials that stay out of request logs and browser history,
 * accepting that a caller cannot paste a working URL into an address bar.
 *
 * We think this is the right trade-off because a leaked token is far more expensive than the
 * convenience of a URL that works on its own.
 */
export function call() {}
