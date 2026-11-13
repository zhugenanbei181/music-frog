//! The capture matrix asks the owner for the Connections columns it renders.
//!
//! A frame may only name columns the contract's closed registry owns: a
//! capture that invents one fails loudly instead of silently rendering a
//! column the product never published.
use super::*;

#[test]
fn a_capture_asks_for_the_columns_the_contract_owns() {
    let columns = parse_connection_columns(Some("status,host,duration"))
        .expect("the contract owns these columns");
    assert_eq!(columns.wire_ids(), vec!["status", "host", "duration"]);
}

#[test]
fn an_absent_or_empty_list_keeps_the_readers_default() {
    assert!(parse_connection_columns(None).is_none());
    assert!(parse_connection_columns(Some("   ")).is_none());
}

#[test]
#[should_panic(expected = "the contract does not own")]
fn a_capture_that_names_an_unknown_column_fails_loudly() {
    let _ = parse_connection_columns(Some("status,not_a_column"));
}
