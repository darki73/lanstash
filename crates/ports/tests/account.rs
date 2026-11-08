//! Tests for `lanstash_ports::account`: the trait works as an object, and a
//! platform has no account unless it says so.

use std::sync::Mutex;

use lanstash_ports::account::Account;
use lanstash_ports::platform::Platform;

#[derive(Default)]
struct Fake(Mutex<Vec<&'static str>>);

impl Account for Fake {
    fn view(&self) -> String {
        "{\"account\":null}".into()
    }
    fn start_sign_in(&self) -> Result<(), String> {
        self.0.lock().unwrap().push("start");
        Ok(())
    }
    fn cancel_sign_in(&self) {
        self.0.lock().unwrap().push("cancel");
    }
    fn sign_out(&self, _account: &str) -> Result<(), String> {
        self.0.lock().unwrap().push("out");
        Ok(())
    }
    fn qr_svg(&self) -> Option<String> {
        None
    }
}

#[derive(Debug)]
struct Bare;

impl Platform for Bare {
    fn id(&self) -> &str {
        "bare"
    }
}

#[test]
fn an_account_is_driven_through_the_trait() {
    let f = Fake::default();
    let a: &dyn Account = &f;
    a.start_sign_in().unwrap();
    a.cancel_sign_in();
    a.sign_out("someone").unwrap();
    assert_eq!(a.view(), "{\"account\":null}");
    assert!(a.qr_svg().is_none());
    assert_eq!(*f.0.lock().unwrap(), ["start", "cancel", "out"]);
    assert!(Bare.account().is_none());
}
