use super::*;

#[test]
fn known_instants() {
    assert_eq!(utc(0), "1970-01-01T00:00:00Z");
    assert_eq!(utc(86_399), "1970-01-01T23:59:59Z");
    assert_eq!(utc(951_782_400), "2000-02-29T00:00:00Z");
    assert_eq!(utc(1_794_268_800), "2026-11-10T00:00:00Z");
    assert_eq!(utc(1_794_268_800 + 3_723), "2026-11-10T01:02:03Z");
    assert_eq!(utc(4_107_542_399), "2100-02-28T23:59:59Z");
    assert_eq!(utc(4_107_542_400), "2100-03-01T00:00:00Z");
}

#[test]
fn before_the_epoch() {
    assert_eq!(utc(-1), "1969-12-31T23:59:59Z");
    assert_eq!(utc(-86_400 * 366), "1968-12-31T00:00:00Z");
}

#[test]
fn agrees_with_a_day_by_day_calendar() {
    let months = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let (mut y, mut m, mut d, mut secs) = (1970, 0usize, 1, 0i64);
    while y < 2200 {
        assert_eq!(utc(secs), format!("{y:04}-{:02}-{d:02}T00:00:00Z", m + 1));
        let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
        let len = if m == 1 && leap { 29 } else { months[m] };
        d += 1;
        if d > len {
            d = 1;
            m += 1;
            if m == 12 {
                m = 0;
                y += 1;
            }
        }
        secs += 86_400;
    }
}
