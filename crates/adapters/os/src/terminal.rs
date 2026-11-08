//! [`Prompt`] on the controlling terminal: questions on standard error,
//! answers from standard input; a secret answer is read with echo off
//! (`stty -echo`, then `stty echo` again), so a password does not show.

use std::io::{BufRead, Write};
use std::process::{Command, Stdio};

use lanstash_ports::prompt::Prompt;

/// The terminal the command runs in.
#[derive(Debug, Clone, Copy, Default)]
pub struct Terminal;

/// Turns echo on or off; `false` when there is no terminal (or no `stty`).
fn echo(on: bool) -> bool {
    Command::new("stty")
        .arg(if on { "echo" } else { "-echo" })
        .stdin(Stdio::inherit())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// A line as typed, without its line end.
#[must_use]
pub fn answer(line: &str) -> String {
    line.trim_end_matches(['\r', '\n']).to_string()
}

impl Prompt for Terminal {
    fn ask(&self, question: &str, secret: bool) -> Result<String, String> {
        eprint!("{question}: ");
        let _ = std::io::stderr().flush();
        let hidden = secret && echo(false);
        let mut line = String::new();
        let read = std::io::stdin().lock().read_line(&mut line);
        if hidden {
            echo(true);
            eprintln!();
        }
        match read {
            Ok(0) => Err(format!("no answer to {question:?} (end of input)")),
            Ok(_) => Ok(answer(&line)),
            Err(e) => Err(format!("reading the answer: {e}")),
        }
    }

    fn tell(&self, message: &str) {
        eprintln!("{message}");
    }
}
