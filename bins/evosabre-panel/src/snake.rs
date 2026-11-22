//! The snake: what the screen can show, in place of the clock, while the
//! player rests.
//!
//! The whole glass is its field: sixty-four cells by sixteen, four pixels
//! to a cell. Nobody plays it; it plays itself. It goes for a note, and a
//! note it has eaten makes it longer. When it has no way left to go it
//! stops for a moment and starts over, short again.
//!
//! It finds its way as a careful player does. It takes the shortest way to
//! the note if, having eaten, it could still reach its own tail, which is
//! the one place that is always about to come free. Otherwise it follows
//! its tail the long way round and looks again at the next step. A snake
//! that has gone hungry for a long time stops being careful, so that it
//! never circles for ever.

use crate::frame::Frame;
use crate::grey;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

pub const COLUMNS: usize = 64;
pub const ROWS: usize = 16;
const CELLS: usize = COLUMNS * ROWS;
/// A cell is four pixels each way: three lit, and one dark between it and
/// the next. The last column and the last row of the glass are so never
/// lit, and the pixel orbit cuts nothing.
const PITCH: u32 = 4;
const DOT: u32 = 3;

/// How long the snake takes over one cell.
pub const STEP: Duration = Duration::from_millis(100);
/// How long it lies where it ran out of room before it starts over.
const REST: Duration = Duration::from_secs(2);
/// Its length when it starts, and the cells a note adds.
const START: usize = 6;
const GROWS: usize = 4;
/// The steps without a note after which it stops being careful.
const PATIENCE: u32 = 4 * CELLS as u32;

const HEAD: u8 = grey::FULL;
const NECK: u8 = grey::linear(12);
const BODY: u8 = grey::linear(10);
const TAIL: u8 = grey::linear(5);
const NOTE: u8 = grey::linear(12);
/// The cells behind the head drawn as its neck, and the last ones drawn
/// fainter, as its tail.
const NECK_CELLS: usize = 2;
const TAIL_CELLS: usize = 6;

/// The note, in the seven pixels by seven that two cells by two have.
const NOTE_ROWS: [&str; 7] = [
    "...XX..", "...X.X.", "...X...", "...X...", ".XXX...", "XXXX...", ".XX....",
];

/// No way from one cell to another.
const NONE: u16 = u16::MAX;

type Cell = usize;

fn cell(x: usize, y: usize) -> Cell {
    y * COLUMNS + x
}

fn column(c: Cell) -> usize {
    c % COLUMNS
}

fn row(c: Cell) -> usize {
    c / COLUMNS
}

/// The cells beside a cell: up to four, fewer at an edge of the field.
fn beside(c: Cell) -> impl Iterator<Item = Cell> {
    let (x, y) = (column(c), row(c));
    [
        (x > 0).then(|| c - 1),
        (x + 1 < COLUMNS).then(|| c + 1),
        (y > 0).then(|| c - COLUMNS),
        (y + 1 < ROWS).then(|| c + COLUMNS),
    ]
    .into_iter()
    .flatten()
}

/// The steps from the nearest of `from` to every cell, going only over
/// cells that are not `taken`; [`NONE`] where there is no way.
fn steps(from: &[Cell], taken: &[bool; CELLS]) -> [u16; CELLS] {
    let mut far = [NONE; CELLS];
    let mut queue = VecDeque::with_capacity(CELLS);
    for &c in from {
        far[c] = 0;
        queue.push_back(c);
    }
    while let Some(c) = queue.pop_front() {
        for next in beside(c) {
            if !taken[next] && far[next] == NONE {
                far[next] = far[c] + 1;
                queue.push_back(next);
            }
        }
    }
    far
}

/// A snake's cells, the head first, and the cells it has still to grow by.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Body {
    cells: VecDeque<Cell>,
    owed: usize,
}

impl Body {
    fn head(&self) -> Cell {
        self.cells[0]
    }

    fn tail(&self) -> Cell {
        self.cells[self.cells.len() - 1]
    }

    /// Move the head to `to`, a cell beside it. A snake that owes no
    /// growth draws its tail in behind it; `ate` is whether there was a
    /// note there.
    fn go(&mut self, to: Cell, ate: bool) {
        self.cells.push_front(to);
        if ate {
            self.owed += GROWS;
        }
        if self.owed > 0 {
            self.owed -= 1;
        } else {
            self.cells.pop_back();
        }
    }

    /// The cells it is in. With `at_next_step`, the cells it will still be
    /// in once it has moved: all of them, but for the tail of a snake that
    /// owes no growth.
    fn taken(&self, at_next_step: bool) -> [bool; CELLS] {
        let mut taken = [false; CELLS];
        for &c in &self.cells {
            taken[c] = true;
        }
        if at_next_step && self.owed == 0 {
            taken[self.tail()] = false;
        }
        taken
    }

    /// How far its head is from its tail by the way that is open to it, or
    /// [`NONE`] if it is shut in.
    fn to_tail(&self) -> u16 {
        let mut taken = self.taken(false);
        taken[self.tail()] = false;
        let far = steps(&[self.tail()], &taken);
        beside(self.head())
            .map(|c| far[c])
            .filter(|&d| d != NONE)
            .min()
            .map_or(NONE, |d| d + 1)
    }
}

#[derive(Debug, Clone)]
pub struct Snake {
    body: Body,
    /// The first cell of the note: it has that one, the one beside it and
    /// the two under them.
    note: Cell,
    /// Steps since it last ate.
    hungry: u32,
    /// The state of the generator the notes are placed by.
    random: u64,
    stepped: Option<Instant>,
    /// When it ran out of room, while it lies there.
    stuck: Option<Instant>,
}

impl Snake {
    /// A snake at its start. `seed` decides where its notes fall.
    pub fn new(seed: u64) -> Self {
        let mut snake = Self {
            body: Body {
                cells: VecDeque::new(),
                owed: 0,
            },
            note: 0,
            hungry: 0,
            // The generator must not start at nought.
            random: seed | 1,
            stepped: None,
            stuck: None,
        };
        snake.start();
        snake
    }

    /// A snake whose notes fall where the clock's last digits say.
    pub fn at_this_moment() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0x9E37_79B9, |d| d.as_nanos() as u64);
        Self::new(nanos)
    }

    fn start(&mut self) {
        let y = ROWS / 2;
        self.body = Body {
            cells: (0..START).map(|i| cell(START + 1 - i, y)).collect(),
            owed: 0,
        };
        self.hungry = 0;
        self.stuck = None;
        // A field with six cells taken has room for a note.
        self.place_note();
    }

    /// How many cells long it is.
    pub fn len(&self) -> usize {
        self.body.cells.len()
    }

    /// The four cells of the note.
    fn note_cells(&self) -> [Cell; 4] {
        [
            self.note,
            self.note + 1,
            self.note + COLUMNS,
            self.note + COLUMNS + 1,
        ]
    }

    fn next_random(&mut self) -> u64 {
        // Xorshift: enough for where a note falls.
        self.random ^= self.random << 13;
        self.random ^= self.random >> 7;
        self.random ^= self.random << 17;
        self.random
    }

    /// Put a note where the snake is not. False when there is no room left
    /// for one.
    fn place_note(&mut self) -> bool {
        let taken = self.body.taken(false);
        let free = |first: Cell| {
            [first, first + 1, first + COLUMNS, first + COLUMNS + 1]
                .iter()
                .all(|&c| !taken[c])
        };
        // Any first cell but those of the last column and the last row.
        let places = (COLUMNS - 1) * (ROWS - 1);
        let from = (self.next_random() % places as u64) as usize;
        for i in 0..places {
            let n = (from + i) % places;
            let first = cell(n % (COLUMNS - 1), n / (COLUMNS - 1));
            if free(first) {
                self.note = first;
                return true;
            }
        }
        false
    }

    /// Whether a snake that sets off from its head to `first`, and on by
    /// the shortest way to the note, could reach its tail once it had
    /// eaten. `to_note` is every cell's distance from the note.
    fn safe_to_eat(&self, first: Cell, to_note: &[u16; CELLS]) -> bool {
        let mut body = self.body.clone();
        let mut at = first;
        loop {
            let ate = to_note[at] == 0;
            body.go(at, ate);
            if ate {
                return body.to_tail() != NONE;
            }
            let Some(next) = beside(at).find(|&c| to_note[c] == to_note[at] - 1) else {
                return false;
            };
            at = next;
        }
    }

    /// The cell to move the head to, or `None` when every cell beside it
    /// is taken.
    fn choose(&self) -> Option<Cell> {
        let taken = self.body.taken(true);
        let open: Vec<Cell> = beside(self.body.head()).filter(|&c| !taken[c]).collect();
        let to_note = steps(&self.note_cells(), &taken);

        // The shortest way to the note, if it is safe to have eaten.
        let nearest = open
            .iter()
            .copied()
            .filter(|&c| to_note[c] != NONE)
            .min_by_key(|&c| to_note[c]);
        if let Some(first) = nearest {
            if self.hungry >= PATIENCE || self.safe_to_eat(first, &to_note) {
                return Some(first);
            }
        }

        // Not now: after its tail, by the way that keeps it longest from it.
        let waiting = open
            .iter()
            .copied()
            .filter_map(|c| {
                let mut body = self.body.clone();
                body.go(c, to_note[c] == 0);
                let far = body.to_tail();
                (far != NONE).then_some((far, c))
            })
            .max_by_key(|&(far, _)| far);
        if let Some((_, c)) = waiting {
            return Some(c);
        }

        // Shut in: into whichever cell has the most room behind it.
        open.iter()
            .copied()
            .max_by_key(|&c| steps(&[c], &taken).iter().filter(|&&d| d != NONE).count())
    }

    /// One step. False when it has run out of room: no cell to move to, or
    /// nowhere to put the next note.
    fn step(&mut self) -> bool {
        let Some(next) = self.choose() else {
            return false;
        };
        let ate = self.note_cells().contains(&next);
        self.body.go(next, ate);
        if ate {
            self.hungry = 0;
            self.place_note()
        } else {
            self.hungry += 1;
            true
        }
    }

    /// Move it on, if it is time: a cell every [`STEP`], and a new start a
    /// while after it ran out of room.
    pub fn tick(&mut self, now: Instant) {
        if let Some(since) = self.stuck {
            if now.saturating_duration_since(since) >= REST {
                self.start();
                self.stepped = Some(now);
            }
            return;
        }
        let due = self
            .stepped
            .is_none_or(|at| now.saturating_duration_since(at) >= STEP);
        if due {
            self.stepped = Some(now);
            if !self.step() {
                self.stuck = Some(now);
            }
        }
    }

    /// How long from `now` until it next has something to do.
    pub fn due_in(&self, now: Instant) -> Duration {
        let (since, after) = match (self.stuck, self.stepped) {
            (Some(stuck), _) => (stuck, REST),
            (None, Some(stepped)) => (stepped, STEP),
            (None, None) => return Duration::ZERO,
        };
        after.saturating_sub(now.saturating_duration_since(since))
    }

    /// The snake and its note, alone on the glass. Where it lies out of
    /// room it is drawn as faint as its tail.
    pub fn draw(&self, f: &mut Frame) {
        let (nx, ny) = (
            column(self.note) as u32 * PITCH,
            row(self.note) as u32 * PITCH,
        );
        for (y, line) in NOTE_ROWS.iter().enumerate() {
            for (x, c) in line.bytes().enumerate() {
                if c == b'X' {
                    f.set(nx + x as u32, ny + y as u32, NOTE);
                }
            }
        }
        let length = self.len();
        for (i, &c) in self.body.cells.iter().enumerate() {
            let luma = if self.stuck.is_some() || i + TAIL_CELLS >= length {
                TAIL
            } else if i == 0 {
                HEAD
            } else if i <= NECK_CELLS {
                NECK
            } else {
                BODY
            };
            let (x, y) = (column(c) as u32 * PITCH, row(c) as u32 * PITCH);
            for dy in 0..DOT {
                for dx in 0..DOT {
                    f.set(x + dx, y + dy, luma);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame;

    /// What must hold of a snake whatever it has done.
    fn sound(snake: &Snake) {
        let cells = &snake.body.cells;
        let mut seen = [false; CELLS];
        for (i, &c) in cells.iter().enumerate() {
            assert!(c < CELLS, "a cell off the field");
            assert!(!seen[c], "two of its cells in one place");
            seen[c] = true;
            if i > 0 {
                assert!(
                    beside(cells[i - 1]).any(|b| b == c),
                    "two of its cells that do not touch"
                );
            }
        }
        if snake.stuck.is_none() {
            let note = snake.note_cells();
            assert!(column(snake.note) + 1 < COLUMNS && row(snake.note) + 1 < ROWS);
            assert!(note.iter().all(|&c| !seen[c]), "the note under the snake");
        }
    }

    /// Step a snake until it has run out of room or `most` steps are done.
    /// Returns the steps taken and the notes eaten.
    fn play(snake: &mut Snake, most: u32) -> (u32, u32) {
        let (mut taken, mut eaten) = (0, 0);
        while taken < most {
            let before = (snake.len(), snake.body.owed);
            let head = snake.body.head();
            if !snake.step() {
                break;
            }
            taken += 1;
            sound(snake);
            assert!(
                beside(head).any(|c| c == snake.body.head()),
                "it moved one cell"
            );
            if snake.hungry == 0 {
                eaten += 1;
                assert_eq!(snake.body.owed, before.1 + GROWS - 1);
                assert_eq!(snake.len(), before.0 + 1);
            } else if before.1 > 0 {
                assert_eq!(snake.len(), before.0 + 1, "it grows by what it owes");
            } else {
                assert_eq!(snake.len(), before.0, "it grows only by eating");
            }
        }
        (taken, eaten)
    }

    #[test]
    fn it_starts_short_in_the_middle_with_a_note_to_go_for() {
        let snake = Snake::new(7);
        assert_eq!(snake.len(), START);
        assert_eq!(snake.body.head(), cell(START + 1, ROWS / 2));
        assert_eq!(snake.body.tail(), cell(2, ROWS / 2));
        sound(&snake);
        // The same seed, the same game; another seed, another note.
        assert_eq!(Snake::new(7).note, snake.note);
        assert!((1..40).any(|seed| Snake::new(seed).note != snake.note));
    }

    #[test]
    fn it_moves_a_cell_at_a_time_eats_and_grows_and_never_crosses_itself() {
        for seed in 1..=3 {
            let mut snake = Snake::new(seed * 7919);
            let (taken, eaten) = play(&mut snake, 1500);
            // A note is four cells' growth, of which some may still be owed.
            assert_eq!(
                snake.len() + snake.body.owed,
                START + GROWS * eaten as usize
            );
            // It is no use as a thing to look at if it dies young or goes
            // hungry: in two and a half minutes it has eaten well and is
            // some way to covering the glass.
            assert!(
                snake.len() >= 120,
                "seed {seed}: {} cells after {taken} steps",
                snake.len()
            );
        }
    }

    #[test]
    fn a_snake_with_no_cell_to_move_to_has_run_out_of_room() {
        // Coiled into the corner, its head shut in by its own body.
        let mut snake = Snake::new(3);
        snake.body = Body {
            cells: [
                cell(0, 0),
                cell(1, 0),
                cell(1, 1),
                cell(0, 1),
                cell(0, 2),
                cell(1, 2),
            ]
            .into_iter()
            .collect(),
            owed: 3,
        };
        snake.note = cell(30, 8);
        assert_eq!(snake.choose(), None);
        assert!(!snake.step());
    }

    #[test]
    fn its_tail_makes_room_as_it_moves_unless_it_is_growing() {
        // A ring of four: the head may step where the tail is leaving.
        let ring = [cell(0, 0), cell(1, 0), cell(1, 1), cell(0, 1)];
        let mut snake = Snake::new(3);
        snake.body = Body {
            cells: ring.into_iter().collect(),
            owed: 0,
        };
        snake.note = cell(30, 8);
        // Walled in on two sides by the field's corner and on two by itself.
        assert_eq!(snake.choose(), Some(cell(0, 1)));
        assert!(snake.step());
        sound(&snake);
        assert_eq!(snake.len(), 4);
        // Growing, the tail stays, and there is no cell to go to.
        snake.body = Body {
            cells: ring.into_iter().collect(),
            owed: 1,
        };
        assert_eq!(snake.choose(), None);
    }

    #[test]
    fn it_does_not_eat_where_it_would_be_shut_in_and_takes_the_long_way() {
        // The note lies in a pocket with one way in, which the snake's own
        // body would close behind it: a careful snake leaves it for now.
        let mut snake = Snake::new(3);
        // A wall along row 2 from column 0 to 5, down column 5 to row 0
        // but for the gap at (5, 1)... drawn as the snake's own body.
        let mut cells: Vec<Cell> = Vec::new();
        cells.push(cell(4, 3)); // head, under the wall
        cells.extend((0..=3).rev().map(|x| cell(x, 3)));
        cells.push(cell(0, 2));
        cells.extend((1..=3).map(|x| cell(x, 2)));
        snake.body = Body {
            cells: cells.into_iter().collect(),
            owed: 0,
        };
        snake.note = cell(0, 0);
        sound(&snake);
        // Its choice keeps its tail within reach.
        let next = snake.choose().unwrap();
        let mut after = snake.body.clone();
        after.go(next, false);
        assert_ne!(after.to_tail(), NONE);
    }

    #[test]
    fn a_snake_gone_hungry_too_long_goes_for_the_note_whatever_comes_of_it() {
        let mut snake = Snake::new(11);
        let to_note = steps(&snake.note_cells(), &snake.body.taken(true));
        let nearest = beside(snake.body.head())
            .filter(|&c| !snake.body.taken(true)[c] && to_note[c] != NONE)
            .min_by_key(|&c| to_note[c])
            .unwrap();
        snake.hungry = PATIENCE;
        assert_eq!(snake.choose(), Some(nearest));
    }

    #[test]
    fn it_steps_at_its_own_pace_and_starts_over_a_while_after_running_out_of_room() {
        let t0 = Instant::now();
        let mut snake = Snake::new(5);
        assert_eq!(snake.due_in(t0), Duration::ZERO);
        let head = snake.body.head();
        snake.tick(t0);
        assert_ne!(snake.body.head(), head, "the first step at once");
        let head = snake.body.head();
        snake.tick(t0 + STEP / 2);
        assert_eq!(snake.body.head(), head, "not before its pace");
        assert_eq!(snake.due_in(t0 + STEP / 2), STEP / 2);
        snake.tick(t0 + STEP);
        assert_ne!(snake.body.head(), head);

        // Shut in: it lies there, then starts over, short again.
        snake.body = Body {
            cells: [
                cell(0, 0),
                cell(1, 0),
                cell(1, 1),
                cell(0, 1),
                cell(0, 2),
                cell(1, 2),
                cell(2, 2),
            ]
            .into_iter()
            .collect(),
            owed: 3,
        };
        let t1 = t0 + STEP * 2;
        snake.tick(t1);
        assert_eq!(snake.stuck, Some(t1));
        assert_eq!(snake.len(), 7);
        snake.tick(t1 + REST / 2);
        assert_eq!(snake.len(), 7, "still lying there");
        assert_eq!(snake.due_in(t1 + REST / 2), REST / 2);
        snake.tick(t1 + REST);
        assert_eq!(snake.len(), START);
        assert_eq!(snake.stuck, None);
        sound(&snake);
    }

    #[test]
    fn it_is_drawn_from_edge_to_edge_and_leaves_the_orbits_column_and_row_dark() {
        let mut snake = Snake::new(9);
        // Along the whole bottom row and up the last column.
        let mut cells: Vec<Cell> = (0..ROWS).map(|y| cell(COLUMNS - 1, y)).collect();
        cells.extend((0..COLUMNS - 1).rev().map(|x| cell(x, ROWS - 1)));
        snake.body = Body {
            cells: cells.into_iter().collect(),
            owed: 0,
        };
        snake.note = cell(COLUMNS - 3, ROWS - 3);
        let mut f = Frame::new();
        snake.draw(&mut f);
        // The head in the top right corner, three pixels square and full.
        assert_eq!(f.get(252, 0), HEAD);
        assert_eq!(f.get(254, 2), HEAD);
        assert_eq!(f.get(251, 0), 0, "a dark pixel between two cells");
        assert_eq!(f.get(252, 4), NECK);
        assert_eq!(f.get(252, 12), BODY);
        // The far end of it, at the left of the bottom row, is its tail.
        assert_eq!(f.get(0, 60), TAIL);
        assert_eq!(f.get(2, 62), TAIL);
        assert_eq!(f.get(4 * TAIL_CELLS as u32, 60), BODY);
        assert!((0..frame::HEIGHT).all(|y| f.get(255, y) == 0), "column 255");
        assert!((0..frame::WIDTH).all(|x| f.get(x, 63) == 0), "row 63");
        // The note, in its two cells by two and no further.
        let (nx, ny) = (4 * (COLUMNS as u32 - 3), 4 * (ROWS as u32 - 3));
        assert_eq!(f.get(nx + 3, ny), NOTE);
        assert_eq!(f.get(nx, ny + 5), NOTE);
        let lit = (0..8)
            .flat_map(|y| (0..8).map(move |x| (x, y)))
            .filter(|&(x, y)| f.get(nx + x, ny + y) == NOTE)
            .count();
        assert_eq!(lit, 15);
        assert!((0..8).all(|i| f.get(nx + 7, ny + i) == 0 && f.get(nx + i, ny + 7) == 0));

        // Out of room, it is all as faint as its tail.
        snake.stuck = Some(Instant::now());
        let mut f = Frame::new();
        snake.draw(&mut f);
        assert_eq!(f.get(252, 0), TAIL);
    }

    #[test]
    fn a_field_with_no_room_for_a_note_is_out_of_room() {
        let mut snake = Snake::new(9);
        // Every other row taken from edge to edge: no two rows free
        // together anywhere.
        let mut cells = Vec::new();
        for y in (0..ROWS).step_by(2) {
            let row: Vec<Cell> = (0..COLUMNS).map(|x| cell(x, y)).collect();
            cells.extend(row);
        }
        snake.body.cells = cells.into_iter().collect();
        assert!(!snake.place_note());
    }
}
