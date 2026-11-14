//! Which of the two pages is up, and the slide from one to the other.
//!
//! The second page is the first moved up until the track's band is at the
//! top. Turning the page is that movement, eased at both ends, and turning
//! it back is the same movement the other way.

use crate::canvas::HEIGHT;
use crate::screens::DETAILS_BAND;
use std::time::{Duration, Instant};

/// How far the first page moves to become the second.
pub const TRAVEL: u32 = HEIGHT - DETAILS_BAND.h;
/// How long the slide takes.
pub const SLIDE: Duration = Duration::from_millis(700);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    First,
    Second,
}

#[derive(Debug, Clone, Copy)]
pub struct Pages {
    on: Page,
    /// When the slide to `on` began; `None` once it has arrived.
    since: Option<Instant>,
}

impl Default for Pages {
    fn default() -> Self {
        Self {
            on: Page::First,
            since: None,
        }
    }
}

impl Pages {
    /// The page that is up, or that the slide is on its way to.
    pub fn on(&self) -> Page {
        self.on
    }

    /// Turn to the other page, sliding. Asked in the middle of a slide, the
    /// page turns back from where it has got to.
    pub fn turn(&mut self, now: Instant) {
        let done = self.fraction(now);
        self.on = match self.on {
            Page::First => Page::Second,
            Page::Second => Page::First,
        };
        // What was done of the old slide is what is left of the new.
        let left = 1000 - done;
        self.since = Some(now - SLIDE * left / 1000);
    }

    /// Back to the first page at once, without a slide.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// How much of the slide is done, in thousandths.
    fn fraction(&self, now: Instant) -> u32 {
        match self.since {
            None => 1000,
            Some(since) => {
                let gone = now.saturating_duration_since(since);
                (gone.as_millis() * 1000 / SLIDE.as_millis()).min(1000) as u32
            }
        }
    }

    /// True while the page is on its way.
    pub fn sliding(&mut self, now: Instant) -> bool {
        if self.since.is_some() && self.fraction(now) >= 1000 {
            self.since = None;
        }
        self.since.is_some()
    }

    /// How far up the first page is drawn at `now`: nought when it is the
    /// page that is up, [`TRAVEL`] when the second is.
    pub fn offset(&mut self, now: Instant) -> u32 {
        let t = u64::from(self.fraction(now));
        // Slow, fast, slow: 3t² - 2t³, in thousandths.
        let eased = (t * t * (3000 - 2 * t) / 1_000_000) as u32;
        let moved = TRAVEL * eased / 1000;
        self.sliding(now);
        match self.on {
            Page::Second => moved,
            Page::First => TRAVEL - moved,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_page_is_up_until_the_page_is_turned() {
        let t0 = Instant::now();
        let mut pages = Pages::default();
        assert_eq!(pages.on(), Page::First);
        assert_eq!(pages.offset(t0), 0);
        assert!(!pages.sliding(t0));
    }

    #[test]
    fn a_turn_slides_slowly_quickly_slowly_and_arrives() {
        let t0 = Instant::now();
        let mut pages = Pages::default();
        pages.turn(t0);
        assert_eq!(pages.on(), Page::Second);
        assert_eq!(pages.offset(t0), 0);
        assert!(pages.sliding(t0));
        let tenth = pages.offset(t0 + SLIDE / 10);
        let half = pages.offset(t0 + SLIDE / 2);
        let nine = pages.offset(t0 + SLIDE * 9 / 10);
        assert!(tenth < TRAVEL / 10, "slow at first: {tenth}");
        assert_eq!(half, TRAVEL / 2);
        assert!(
            nine > TRAVEL * 9 / 10 && nine < TRAVEL,
            "slow at the end: {nine}"
        );
        assert_eq!(pages.offset(t0 + SLIDE), TRAVEL);
        assert!(!pages.sliding(t0 + SLIDE));
        assert_eq!(pages.offset(t0 + SLIDE * 5), TRAVEL);
    }

    #[test]
    fn turning_back_is_the_same_slide_the_other_way() {
        let t0 = Instant::now();
        let mut pages = Pages::default();
        pages.turn(t0);
        let there = t0 + SLIDE;
        assert_eq!(pages.offset(there), TRAVEL);
        pages.turn(there);
        assert_eq!(pages.on(), Page::First);
        assert_eq!(pages.offset(there), TRAVEL);
        assert_eq!(pages.offset(there + SLIDE / 2), TRAVEL / 2);
        assert_eq!(pages.offset(there + SLIDE), 0);
    }

    #[test]
    fn turned_in_the_middle_of_a_slide_the_page_goes_back_from_where_it_is() {
        let t0 = Instant::now();
        let mut pages = Pages::default();
        pages.turn(t0);
        let middle = t0 + SLIDE / 2;
        let was = pages.offset(middle);
        pages.turn(middle);
        assert_eq!(pages.offset(middle), was, "no jump");
        assert_eq!(pages.offset(middle + SLIDE / 2), 0);
    }

    #[test]
    fn a_reset_is_the_first_page_at_once() {
        let t0 = Instant::now();
        let mut pages = Pages::default();
        pages.turn(t0);
        pages.reset();
        assert_eq!(pages.on(), Page::First);
        assert_eq!(pages.offset(t0), 0);
        assert!(!pages.sliding(t0));
    }
}
