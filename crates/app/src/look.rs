//! Waiting for something to change: a background job that works out what
//! depends on, say, a platform's part of the stash looks at a cheap mark of it
//! every so often and goes on as soon as the mark moved, or after a longest
//! wait all the same.

use lanstash_ports::clock::Pause;

/// Returns once `mark()` differs from `before`, looking every `look_ms`, or after about
/// `longest_ms`; whether it moved.
pub fn until_changed<M: PartialEq>(
    pause: &dyn Pause,
    look_ms: u64,
    longest_ms: u64,
    before: &M,
    mark: &dyn Fn() -> M,
) -> bool {
    let mut waited = 0;
    while waited < longest_ms {
        pause.pause_ms(look_ms.min(longest_ms - waited));
        waited += look_ms.min(longest_ms - waited);
        if mark() != *before {
            return true;
        }
    }
    false
}
