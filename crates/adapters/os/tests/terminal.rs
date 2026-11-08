//! Tests for `lanstash_adapter_os::terminal` (the reading itself needs a person).

use lanstash_adapter_os::terminal::{Terminal, answer};
use lanstash_ports::prompt::Prompt;

#[test]
fn an_answer_loses_only_its_line_end() {
    assert_eq!(answer("AB12C\r\n"), "AB12C");
    assert_eq!(answer("  pass word \n"), "  pass word ");
    assert_eq!(answer(""), "");
}

#[test]
fn the_terminal_is_a_prompt() {
    let p: &dyn Prompt = &Terminal;
    p.tell("(a message the test shows on standard error)");
}
