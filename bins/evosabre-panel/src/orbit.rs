//! The pixel orbit: the whole picture moves one pixel around a square,
//! one step a minute, so no pixel carries the same content for long. The
//! layout keeps a spare column and row, so nothing is ever cut off.

use std::time::Duration;

pub const STEP: Duration = Duration::from_secs(60);

pub fn offset(since_start: Duration) -> (u32, u32) {
    match (since_start.as_secs() / STEP.as_secs()) % 4 {
        0 => (0, 0),
        1 => (1, 0),
        2 => (1, 1),
        _ => (0, 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_square_one_step_a_minute() {
        let m = |n: u64| offset(Duration::from_secs(60 * n + 5));
        assert_eq!(m(0), (0, 0));
        assert_eq!(m(1), (1, 0));
        assert_eq!(m(2), (1, 1));
        assert_eq!(m(3), (0, 1));
        assert_eq!(m(4), (0, 0));
        assert_eq!(offset(Duration::from_secs(59)), (0, 0));
    }
}
