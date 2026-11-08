//! Runs an external program (`df`, `systemctl`) with a time limit: a hung
//! program (a stuck network mount, a busy init system) is killed, never waited
//! for forever.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::Duration;

/// How often a running program is checked.
const POLL: Duration = Duration::from_millis(20);

/// Exit success and standard output of a finished program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub success: bool,
    pub code: Option<i32>,
    pub stdout: String,
}

/// Runs `program args…` and waits up to `timeout`. Standard error is
/// discarded; outputs are expected to be small (they are read at the end).
///
/// # Errors
/// The program can't be started, or did not finish within `timeout` (it is killed).
pub fn run(program: &str, args: &[&str], timeout: Duration) -> Result<Output, String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("{program}: {e}"))?;
    // Waits in steps instead of reading a clock (only the composition root does).
    let mut waited = Duration::ZERO;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if waited < timeout => {
                std::thread::sleep(POLL);
                waited += POLL;
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{program}: no answer within {timeout:?}"));
            }
            Err(e) => return Err(format!("{program}: {e}")),
        }
    };
    let mut stdout = String::new();
    if let Some(mut o) = child.stdout.take() {
        o.read_to_string(&mut stdout).map_err(|e| format!("{program}: {e}"))?;
    }
    Ok(Output { success: status.success(), code: status.code(), stdout })
}
