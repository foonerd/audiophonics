//! Local time for the idle screen, from the unit's own zone file, with no
//! libc call: the runtime forbids unsafe code, and tz-rs parses TZif itself.

use tz::TimeZone;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalTime {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    /// Days since Sunday.
    pub weekday: u8,
}

impl LocalTime {
    pub fn hhmm(&self) -> String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }

    pub fn weekday_name(&self) -> &'static str {
        [
            "Sunday",
            "Monday",
            "Tuesday",
            "Wednesday",
            "Thursday",
            "Friday",
            "Saturday",
        ][usize::from(self.weekday % 7)]
    }

    pub fn date_line(&self) -> String {
        const MONTHS: [&str; 12] = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];
        let month = MONTHS[usize::from(self.month.clamp(1, 12) - 1)];
        format!("{} {} {}", self.day, month, self.year)
    }
}

pub struct Clock {
    zone: Option<TimeZone>,
}

impl Clock {
    /// The zone is read once, at start. A unit whose zone cannot be read
    /// shows UTC and says so in the journal.
    pub fn new() -> (Self, Option<String>) {
        match TimeZone::local() {
            Ok(zone) => (Self { zone: Some(zone) }, None),
            Err(e) => (
                Self { zone: None },
                Some(format!("time zone unreadable ({e}); showing UTC")),
            ),
        }
    }

    pub fn now(&self) -> Option<LocalTime> {
        let utc = TimeZone::utc();
        let zone = self.zone.as_ref().map_or(utc.as_ref(), |z| z.as_ref());
        let t = tz::DateTime::now(zone).ok()?;
        Some(LocalTime {
            year: t.year(),
            month: t.month(),
            day: t.month_day(),
            hour: t.hour(),
            minute: t.minute(),
            weekday: t.week_day(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lines_read_as_a_person_writes_them() {
        let t = LocalTime {
            year: 2026,
            month: 9,
            day: 16,
            hour: 14,
            minute: 7,
            weekday: 3,
        };
        assert_eq!(t.hhmm(), "14:07");
        assert_eq!(t.weekday_name(), "Wednesday");
        assert_eq!(t.date_line(), "16 September 2026");
    }

    #[test]
    fn the_clock_answers_on_this_machine() {
        let (clock, _) = Clock::new();
        let t = clock.now().unwrap();
        assert!(t.hour < 24 && t.minute < 60 && (1..=12).contains(&t.month));
    }
}
