//! Tests for `lanstash_ports::prompt`: the trait works as an object.

use std::sync::Mutex;

use lanstash_ports::prompt::Prompt;

#[derive(Default)]
struct Scripted(Mutex<Vec<String>>);

impl Prompt for Scripted {
    fn ask(&self, question: &str, secret: bool) -> Result<String, String> {
        Ok(format!("{question}:{secret}"))
    }
    fn tell(&self, message: &str) {
        self.0.lock().unwrap().push(message.into());
    }
}

#[test]
fn questions_get_answers_and_messages_are_shown() {
    let s = Scripted::default();
    let p: &dyn Prompt = &s;
    assert_eq!(p.ask("Password", true).unwrap(), "Password:true");
    p.tell("logged on");
    assert_eq!(*s.0.lock().unwrap(), ["logged on"]);
}
