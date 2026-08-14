// A fixture for yadr's end-to-end tests. This file is never compiled; the test suite only ever
// runs the `yadr` binary over it and asserts on what comes out. Two Y-Statements live here so that
// the tests cover a file containing more than one, and they use different comment syntaxes so that
// both of Rust's are covered.

/*
 * YADR: 2024-01-15 <title>
 *
 * In the context of <ctx>, we faced <con>.
 *
 * We decided for <opt>, and neglected <alt>.
 *
 * We did this to achieve <qua>, accepting <dwn>.
 *
 * We think this is the right trade-off because <why>.
 *
 * 2024-02-01: <chg>
 */
fn store() {}

// YADR: 2024-01-22 <title>
//
// In the context of <ctx>, we faced <con>.
//
// We decided for <opt>, and neglected <alt>.
//
// We did this to achieve <qua>, accepting <dwn>.
//
// We think this is the right trade-off because <why>.
fn read() {}
