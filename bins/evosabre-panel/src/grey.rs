//! The panel's greys.
//!
//! The controller shows sixteen levels and is told, for each of the fifteen
//! above black, how long to drive a pixel: so many ticks of its display
//! clock. Its own table is linear, eight ticks a level, and leaves the
//! first level dark, so the faintest grey it has is eight ticks.
//!
//! The faces are drawn on that linear scale and keep it. Two levels are
//! added under it, one and two ticks, for what should be there without
//! drawing the eye: the unlit segments of the clock's digits. Their room
//! comes from the top, where the step before full brightness cannot be told
//! from full brightness.
//!
//! A frame holds levels, 0 to 15. Drawing code names a grey on the linear
//! scale through [`linear`], or one of the two faint levels by name.

/// Ticks of the display clock for levels 1 to 15.
pub const TICKS: [u8; 15] = [1, 2, 8, 16, 24, 32, 40, 48, 56, 64, 72, 80, 88, 96, 112];

/// The faintest the panel is asked for: one tick.
pub const TRACE: u8 = 1;
/// Two ticks: a quarter of the linear scale's first step.
pub const FAINT: u8 = 2;
/// Full brightness.
pub const FULL: u8 = 15;

/// The level for a grey on the linear scale of sixteen, 0 black and 15
/// full. Its first step is too faint for the controller's linear table and
/// is given the nearer of the two added levels; its fourteenth is shown at
/// full.
pub const fn linear(sixteenth: u8) -> u8 {
    match sixteenth {
        0 => 0,
        1 => FAINT,
        2..=13 => sixteenth + 1,
        _ => FULL,
    }
}

/// How bright a level is, 0 to 255, for a picture of a frame.
pub fn brightness(level: u8) -> u8 {
    match level {
        0 => 0,
        1..=15 => {
            let ticks = u32::from(TICKS[usize::from(level) - 1]);
            let full = u32::from(TICKS[14]);
            ((ticks * 255 + full / 2) / full) as u8
        }
        _ => 255,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The controller takes a table only if each level is driven longer
    /// than the one before, and none for more than 180 ticks.
    #[test]
    fn the_table_rises_and_stays_in_the_controllers_range() {
        assert!(TICKS[0] > 0, "the first level is lit");
        assert!(TICKS.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(TICKS.iter().all(|&ticks| ticks <= 180));
    }

    #[test]
    fn the_linear_scale_is_the_controllers_own_eight_ticks_a_step() {
        assert_eq!(linear(0), 0);
        for sixteenth in 2..=13u8 {
            let ticks = TICKS[usize::from(linear(sixteenth)) - 1];
            assert_eq!(ticks, 8 * (sixteenth - 1), "step {sixteenth}");
        }
        assert_eq!(linear(14), FULL);
        assert_eq!(linear(15), FULL);
        assert_eq!(TICKS[usize::from(FULL) - 1], 112);
    }

    #[test]
    fn the_two_faint_levels_sit_under_the_linear_scale_in_order() {
        let ticks = |level: u8| TICKS[usize::from(level) - 1];
        assert!(ticks(TRACE) < ticks(FAINT));
        assert!(ticks(FAINT) < ticks(linear(2)));
        assert_eq!(linear(1), FAINT);
    }

    #[test]
    fn brightness_follows_the_ticks() {
        assert_eq!(brightness(0), 0);
        assert_eq!(brightness(FULL), 255);
        assert_eq!(brightness(linear(8)), ((56 * 255 + 56) / 112) as u8);
        assert!(brightness(TRACE) > 0 && brightness(TRACE) < brightness(FAINT));
    }
}
