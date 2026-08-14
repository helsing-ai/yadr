// A fixture for yadr's end-to-end tests, holding a Y-Statement that deliberately does not follow
// the format. `yadr check` is expected to reject this file.
//
// The second paragraph is the broken one: it says "We chose" where the format calls for "We
// decided for", so the diagnostic should name paragraph 2.

/*
 * YADR: 2024-05-20 <title>
 *
 * In the context of <ctx>, we faced <con>.
 *
 * We chose one of the options available to us, and forgot to write the rest of the format.
 */
fn broken() {}
