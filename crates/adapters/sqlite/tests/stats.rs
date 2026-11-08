//! Tests for `lanstash_adapter_sqlite::stats`: minutes add up, sum into
//! buckets, and the old ones are forgotten.

use lanstash_adapter_sqlite::db::Database;
use lanstash_ports::stats::StatsLog;

const MIN: u64 = 60_000;

#[test]
fn minutes_add_up_and_sum_into_buckets() {
    let db = Database::in_memory().unwrap();
    db.add(0, &[("hits".into(), 2), ("misses".into(), 0)]).unwrap();
    db.add(0, &[("hits".into(), 1)]).unwrap();
    db.add(MIN, &[("hits".into(), 4), ("misses".into(), 1)]).unwrap();
    db.add(5 * MIN, &[("hits".into(), 10), ("other".into(), 99)]).unwrap();
    let names = ["hits".to_string(), "misses".to_string()];
    assert_eq!(
        db.series(&names, 0, 10 * MIN, MIN).unwrap(),
        vec![(0, vec![3, 0]), (MIN, vec![4, 1]), (5 * MIN, vec![10, 0])]
    );
    assert_eq!(
        db.series(&names, 0, 10 * MIN, 5 * MIN).unwrap(),
        vec![(0, vec![7, 1]), (5 * MIN, vec![10, 0])],
        "a bucket sums its minutes; one with nothing is left out"
    );
    assert_eq!(db.series(&names, MIN, 5 * MIN, MIN).unwrap(), vec![(MIN, vec![4, 1])], "from and to bound it");
    assert_eq!(db.series(&[], 0, 10 * MIN, MIN).unwrap(), vec![]);
}

#[test]
fn the_counters_that_went_up_most() {
    let db = Database::in_memory().unwrap();
    db.add(0, &[("served/steam/7".into(), 5), ("served/steam/8".into(), 9), ("requests.hits".into(), 99)]).unwrap();
    db.add(MIN, &[("served/steam/7".into(), 6), ("served/blizzard/osi".into(), 1)]).unwrap();
    db.add(5 * MIN, &[("served/steam/7".into(), 100)]).unwrap();
    assert_eq!(
        db.top("served/", 0, 2 * MIN, 10).unwrap(),
        vec![("served/steam/7".into(), 11), ("served/steam/8".into(), 9), ("served/blizzard/osi".into(), 1)]
    );
    assert_eq!(db.top("served/", 0, 2 * MIN, 1).unwrap(), vec![("served/steam/7".into(), 11)]);
    assert_eq!(db.top("served/steam/", MIN, 10 * MIN, 10).unwrap(), vec![("served/steam/7".into(), 106)]);
}

#[test]
fn old_minutes_are_forgotten() {
    let db = Database::in_memory().unwrap();
    db.add(0, &[("hits".into(), 1)]).unwrap();
    db.add(MIN, &[("hits".into(), 1), ("misses".into(), 1)]).unwrap();
    db.add(2 * MIN, &[("hits".into(), 1)]).unwrap();
    assert_eq!(db.forget_before(2 * MIN).unwrap(), 3);
    assert_eq!(db.series(&["hits".to_string()], 0, 3 * MIN, MIN).unwrap(), vec![(2 * MIN, vec![1])]);
}
