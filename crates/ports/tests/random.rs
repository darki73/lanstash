//! Tests for `lanstash_ports::random`: the trait works as an object.

use lanstash_ports::random::Random;

struct Counting;

impl Random for Counting {
    fn fill(&self, buf: &mut [u8]) -> Result<(), String> {
        for (i, b) in buf.iter_mut().enumerate() {
            *b = i as u8;
        }
        Ok(())
    }
}

#[test]
fn fill_writes_the_whole_buffer() {
    let r: &dyn Random = &Counting;
    let mut b = [9u8; 4];
    r.fill(&mut b).unwrap();
    assert_eq!(b, [0, 1, 2, 3]);
}
