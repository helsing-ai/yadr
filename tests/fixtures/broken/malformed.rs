// A fixture for yadr's end-to-end tests, holding a Y-Statement that deliberately does not follow
// the format. `yadr check` is expected to reject this file.
//
// The second paragraph is the broken one: it says "We chose" where the format calls for "We
// decided for", so the diagnostic should name paragraph 2.

/*
 * YADR: 2024-05-20 A statement that does not follow the format
 *
 * In the context of testing what yadr reports for a malformed statement, we faced the need
 * for an input that is recognisably a Y-Statement but does not parse as one.
 *
 * We chose one of the options available to us, and forgot to write the rest of the format.
 */
fn broken() {}
