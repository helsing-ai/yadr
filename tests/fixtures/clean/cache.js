// A fixture for yadr's end-to-end tests. Never executed; only ever read as text.
//
// The statement is written as a run of `//` lines, which `client.ts` does not cover. Between this
// and the TypeScript fixture, both of the comment forms their closely
// -related grammars share are exercised.

// YADR: 2024-07-23 Key the cache by the full request URL
//
// In the context of caching responses from an API that varies its output on query parameters,
// we faced the question of what to key an entry on.
//
// We decided for keying on the full request URL including its query string, and neglected
// keying on the path alone.
//
// We did this to achieve entries that can never be served for a request that would have
// produced something else, accepting that two requests differing only in parameter order miss
// the cache separately.
//
// We think this is the right trade-off because serving a wrong response is indistinguishable
// from a bug to the caller, whereas an extra miss only costs a round trip.
export function get(url) {
  return url;
}
