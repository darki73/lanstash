use std::sync::Mutex;

use lanstash_ports::stats::{Bucket, StatsLog};

/// The contract as a minimal log: minutes of deltas, summed into buckets.
#[derive(Default)]
struct Minutes(Mutex<Vec<(u64, String, u64)>>);

impl StatsLog for Minutes {
    fn add(&self, minute_ms: u64, deltas: &[(String, u64)]) -> Result<(), String> {
        self.0.lock().unwrap().extend(deltas.iter().map(|(n, d)| (minute_ms, n.clone(), *d)));
        Ok(())
    }
    fn series(&self, names: &[String], from_ms: u64, to_ms: u64, step_ms: u64) -> Result<Vec<Bucket>, String> {
        let rows = self.0.lock().unwrap();
        let mut out: Vec<Bucket> = Vec::new();
        for (m, n, d) in rows.iter().filter(|(m, _, _)| *m >= from_ms && *m < to_ms) {
            let Some(i) = names.iter().position(|x| x == n) else { continue };
            let start = from_ms + (m - from_ms) / step_ms * step_ms;
            if out.last().is_none_or(|b| b.0 != start) {
                out.push((start, vec![0; names.len()]));
            }
            if let Some(b) = out.last_mut() {
                b.1[i] += d;
            }
        }
        Ok(out)
    }
    fn top(&self, prefix: &str, from_ms: u64, to_ms: u64, limit: usize) -> Result<Vec<(String, u64)>, String> {
        let mut sums: Vec<(String, u64)> = Vec::new();
        for (m, n, d) in self.0.lock().unwrap().iter() {
            if *m < from_ms || *m >= to_ms || !n.starts_with(prefix) {
                continue;
            }
            match sums.iter_mut().find(|(x, _)| x == n) {
                Some((_, s)) => *s += d,
                None => sums.push((n.clone(), *d)),
            }
        }
        sums.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        sums.truncate(limit);
        Ok(sums)
    }
    fn forget_before(&self, before_ms: u64) -> Result<u64, String> {
        let mut rows = self.0.lock().unwrap();
        let n = rows.len();
        rows.retain(|(m, _, _)| *m >= before_ms);
        Ok((n - rows.len()) as u64)
    }
}

#[test]
fn minutes_sum_into_buckets() {
    let log = Minutes::default();
    log.add(0, &[("hits".into(), 2), ("misses".into(), 1)]).unwrap();
    log.add(60_000, &[("hits".into(), 3)]).unwrap();
    log.add(120_000, &[("hits".into(), 5)]).unwrap();
    let names = ["hits".to_string(), "misses".to_string()];
    assert_eq!(log.series(&names, 0, 180_000, 120_000).unwrap(), vec![(0, vec![5, 1]), (120_000, vec![5, 0])]);
    assert_eq!(log.top("h", 0, 180_000, 5).unwrap(), vec![("hits".to_string(), 10)]);
    assert_eq!(log.forget_before(60_000).unwrap(), 2);
}
