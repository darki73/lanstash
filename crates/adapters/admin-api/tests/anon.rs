//! Tests for `lanstash_adapter_admin_api::anon`: who the clients are, left out for a guest.

use lanstash_adapter_admin_api::anon::{Pseudonyms, without_clients};

#[test]
fn a_clients_list_loses_names_and_addresses_become_names_of_no_meaning() {
    let names = Pseudonyms::default();
    let status = "{\"clients\":[{\"address\":\"192.0.2.7\",\"label\":\"PC-1\",\"host\":\"pc-1.lan\",\"game\":\"20\",\"per_sec\":5},{\"address\":\"192.0.2.9\",\"game\":\"21\"},{\"address\":\"192.0.2.7\",\"game\":\"22\"}],\"usage\":[{\"game\":\"20\",\"host\":\"kept\"}]}\n";
    assert_eq!(
        without_clients(status, &names),
        "{\"clients\":[{\"address\":\"client-1\",\"game\":\"20\",\"per_sec\":5},{\"address\":\"client-2\",\"game\":\"21\"},{\"address\":\"client-1\",\"game\":\"22\"}],\"usage\":[{\"game\":\"20\",\"host\":\"kept\"}]}\n",
        "the PCs stay apart and countable"
    );
    assert_eq!(names.of("192.0.2.9"), "client-2", "the same name the next time");
    assert_eq!(without_clients("not json", &names), "not json");
}
