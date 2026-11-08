//! Background loops that outlive a round that panics: the panic is reported and the
//! loop goes on after a pause, instead of its thread ending unnoticed.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use lanstash_ports::clock::Pause;

/// How long a loop rests after a round panicked.
pub const AFTER_PANIC_MS: u64 = 60_000;

/// Whether a loop runs another round.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    Again,
    Stop,
}

/// Runs `round` again and again on a thread named `name`, until it answers [`Next::Stop`].
/// A round's own pauses are its business; a round that panics is followed by a pause of
/// [`AFTER_PANIC_MS`].
///
/// # Errors
/// The thread could not be spawned.
pub fn forever(
    name: impl Into<String>,
    pause: Arc<dyn Pause>,
    mut round: impl FnMut() -> Next + Send + 'static,
) -> Result<(), String> {
    let name = name.into();
    std::thread::Builder::new()
        .name(name.clone())
        .spawn(move || {
            loop {
                match catch_unwind(AssertUnwindSafe(&mut round)) {
                    Ok(Next::Again) => {}
                    Ok(Next::Stop) => return,
                    Err(_) => {
                        println!("{name}: a round failed unexpectedly; going on in {} s", AFTER_PANIC_MS / 1000);
                        pause.pause_ms(AFTER_PANIC_MS);
                    }
                }
            }
        })
        .map(drop)
        .map_err(|e| e.to_string())
}
