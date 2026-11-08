//! Who the clients are, left out of what a guest (no one signed in) is answered: in every list
//! under `clients` (the status's PCs downloading now), each entry loses its `label` and `host`, and
//! its `address` becomes a name of no meaning (`client-1`, `client-2`, ... in the order lanstash
//! first saw them, the same for as long as it runs), so the page can still tell the PCs apart and
//! count them. What each downloads, how fast and how much stay.

use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

use lanstash_domain::json::{Value, parse, write};

/// What names a client beside its address.
const NAMING: &[&str] = &["label", "host"];

/// The names of no meaning given to the clients' addresses.
#[derive(Debug, Default)]
pub struct Pseudonyms(Mutex<BTreeMap<String, usize>>);

impl Pseudonyms {
    /// The name `address` goes by: the same each time.
    #[must_use]
    pub fn of(&self, address: &str) -> String {
        let mut known = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let next = known.len() + 1;
        format!("client-{}", *known.entry(address.to_string()).or_insert(next))
    }
}

/// `text` without the clients' addresses and names; what is not JSON comes back as it was.
#[must_use]
pub fn without_clients(text: &str, names: &Pseudonyms) -> String {
    match parse(text) {
        Ok(v) => {
            let out = write(&walk(v, false, names));
            if text.ends_with('\n') { out + "\n" } else { out }
        }
        Err(_) => text.to_string(),
    }
}

/// `v`, with the entries of a `clients` list (`in_clients`) unnamed.
fn walk(v: Value, in_clients: bool, names: &Pseudonyms) -> Value {
    match v {
        Value::Array(items) => Value::Array(items.into_iter().map(|x| walk(x, in_clients, names)).collect()),
        Value::Object(members) => Value::Object(
            members
                .into_iter()
                .filter(|(k, _)| !(in_clients && NAMING.contains(&k.as_str())))
                .map(|(k, v)| match v {
                    Value::Str(a) if in_clients && k == "address" => (k, Value::Str(names.of(&a))),
                    v => {
                        let under = k == "clients";
                        (k, walk(v, under, names))
                    }
                })
                .collect(),
        ),
        other => other,
    }
}
