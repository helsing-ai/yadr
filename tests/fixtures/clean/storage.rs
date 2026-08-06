// A fixture for yadr's end-to-end tests. This file is never compiled; the test suite only ever
// runs the `yadr` binary over it and asserts on what comes out. Two Y-Statements live here so that
// the tests cover a file containing more than one.

/*
 * YADR: 2024-01-15 Store timestamps as UTC
 *
 * In the context of comparing event timestamps recorded on different machines, we faced
 * ambiguity about which offset a bare local timestamp had been written in.
 *
 * We decided for storing every timestamp in UTC, and neglected recording the local offset
 * alongside each timestamp.
 *
 * We did this to achieve unambiguous ordering of events no matter where they were recorded,
 * accepting that rendering a timestamp in the recorder's own time zone needs a separate
 * lookup.
 *
 * We think this is the right trade-off because ordering matters everywhere in the system,
 * whereas local-time rendering matters only in the user interface.
 *
 * 2024-02-01: noted that the user interface is where the offset lookup happens.
 */
fn store() {}

// YADR: 2024-01-22 Fail closed on an unreadable file
//
// In the context of walking a source tree we do not control, we faced the question of what
// to do when a file cannot be read.
//
// We decided for aborting the whole run, and neglected skipping the file with a warning.
//
// We did this to achieve a guarantee that a successful run really did inspect everything,
// accepting that one unreadable file stops the run.
//
// We think this is the right trade-off because a partial pass reported as a success is worse
// than an obvious failure.
fn read() {}
