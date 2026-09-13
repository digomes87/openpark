//! The line for a ride.
//!
//! A queue is an order, not a place: a list of guests, front first. Where the
//! line actually stands is worked out from the land — the run of queue path
//! leading away from the station — so laying a longer queue makes a longer line
//! without anything here needing to know about tiles.
//!
//! A ride with no queue path still has a queue. It is one guest long, standing
//! beside the station, which is the honest version of "you did not build one".

use isogrid::iso::TilePos;
use serde::{Deserialize, Serialize};

use crate::park::{Land, Terrain};

/// The line of guests waiting for one ride.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Queue {
    /// Who is waiting, front of the line first.
    waiting: Vec<u32>,
}

impl Queue {
    /// How long a queue is allowed to get, however much path was laid.
    ///
    /// A cap on the search as much as on the gameplay: the line is walked every
    /// tick, for every ride.
    pub const MAX_LENGTH: usize = 40;

    /// The longest a guest will stand in line before it stops being worth it.
    ///
    /// Three minutes at [`isogrid::time::TickRate::CLASSIC`].
    pub const PATIENCE: u32 = 7_200;

    /// Nobody waiting.
    pub const fn new() -> Self {
        Self {
            waiting: Vec::new(),
        }
    }

    /// How many guests are in line.
    pub fn len(&self) -> usize {
        self.waiting.len()
    }

    /// Whether nobody is waiting.
    pub fn is_empty(&self) -> bool {
        self.waiting.is_empty()
    }

    /// Everybody in line, front first.
    pub fn waiting(&self) -> &[u32] {
        &self.waiting
    }

    /// Whether `guest` is already in this line.
    pub fn holds(&self, guest: u32) -> bool {
        self.waiting.contains(&guest)
    }

    /// Where in the line `guest` is standing, counting from the front.
    pub fn place_of(&self, guest: u32) -> Option<usize> {
        self.waiting.iter().position(|waiting| *waiting == guest)
    }

    /// Puts `guest` on the end of the line.
    ///
    /// Returns whether they joined: a full line turns people away, and nobody
    /// joins a line they are already in.
    ///
    /// ```
    /// # use openpark::park::Queue;
    /// let mut queue = Queue::new();
    /// assert!(queue.join(1));
    /// assert!(!queue.join(1), "nobody queues twice");
    /// assert_eq!(queue.place_of(1), Some(0), "first in line");
    /// ```
    pub fn join(&mut self, guest: u32) -> bool {
        if self.waiting.len() >= Self::MAX_LENGTH || self.holds(guest) {
            return false;
        }

        self.waiting.push(guest);
        true
    }

    /// Takes `guest` out of the line, wherever they were standing.
    ///
    /// Everybody behind them moves up, which is the whole point of keeping the
    /// line as an order rather than a set of positions.
    pub fn leave(&mut self, guest: u32) -> bool {
        let Some(place) = self.place_of(guest) else {
            return false;
        };

        self.waiting.remove(place);
        true
    }

    /// Who is at the front, if anybody is.
    pub fn front(&self) -> Option<u32> {
        self.waiting.first().copied()
    }

    /// Empties the line out, returning everybody who was in it.
    ///
    /// What happens when a ride is shut, breaks down or is demolished: the line
    /// does not stand there waiting for something that is not coming back.
    pub fn send_everybody_away(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.waiting)
    }
}

/// Where the line for a station actually stands, front tile first.
///
/// The run of queue path leading away from `station`, walked one tile at a time.
/// The first tile is the one beside the station — where a guest has to be
/// standing to get on — and the last is the back of the line.
///
/// A station with no queue path beside it gets the walkable tiles next to it
/// instead, so a ride built without a queue still works and simply cannot hold a
/// line.
pub fn line_from(land: &Land, station: TilePos) -> Vec<TilePos> {
    let mut line: Vec<TilePos> = Vec::new();

    let start = station
        .neighbours()
        .into_iter()
        .find(|tile| land.ground(*tile) == Some(Terrain::Queue));

    let Some(start) = start else {
        // No queue laid: one place to stand, beside the station.
        return station
            .neighbours()
            .into_iter()
            .filter(|tile| land.ground(*tile).is_some_and(Terrain::is_walkable))
            .take(1)
            .collect();
    };

    let mut here = start;
    line.push(here);

    while line.len() < Queue::MAX_LENGTH {
        let next = here.neighbours().into_iter().find(|tile| {
            *tile != station && land.ground(*tile) == Some(Terrain::Queue) && !line.contains(tile)
        });

        let Some(next) = next else {
            break;
        };

        line.push(next);
        here = next;
    }

    line
}

#[cfg(test)]
mod tests {
    use super::*;

    use isogrid::grid::Grid;

    fn land_with(queue: &[TilePos]) -> Land {
        let mut land = Land::flat(Grid::filled(16, 16, Terrain::Grass).unwrap()).unwrap();
        for tile in queue {
            land.set_ground(*tile, Terrain::Queue);
        }
        land
    }

    #[test]
    fn a_new_queue_is_empty() {
        let queue = Queue::new();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
        assert_eq!(queue.front(), None);
        assert_eq!(queue.place_of(1), None);
    }

    #[test]
    fn the_line_keeps_the_order_people_joined_in() {
        let mut queue = Queue::new();
        for guest in 1..=4 {
            assert!(queue.join(guest));
        }

        assert_eq!(queue.waiting(), &[1, 2, 3, 4]);
        assert_eq!(queue.front(), Some(1));
        assert_eq!(queue.place_of(3), Some(2));
    }

    #[test]
    fn everybody_behind_somebody_who_leaves_moves_up() {
        let mut queue = Queue::new();
        for guest in 1..=4 {
            queue.join(guest);
        }

        assert!(queue.leave(2));
        assert_eq!(queue.waiting(), &[1, 3, 4]);
        assert_eq!(queue.place_of(3), Some(1), "it moved up one");
        assert!(!queue.leave(2), "it left twice");
    }

    #[test]
    fn a_full_line_turns_people_away() {
        let mut queue = Queue::new();
        for guest in 0..Queue::MAX_LENGTH {
            #[allow(clippy::cast_possible_truncation)]
            let guest = guest as u32;
            assert!(queue.join(guest), "the line filled up early");
        }

        assert!(!queue.join(9_999), "the line took one too many");
        assert_eq!(queue.len(), Queue::MAX_LENGTH);
    }

    #[test]
    fn shutting_a_ride_sends_the_whole_line_away() {
        let mut queue = Queue::new();
        for guest in 1..=3 {
            queue.join(guest);
        }

        assert_eq!(queue.send_everybody_away(), vec![1, 2, 3]);
        assert!(queue.is_empty());
    }

    #[test]
    fn the_line_stands_along_the_queue_path() {
        let station = TilePos::new(5, 5);
        let laid = [
            TilePos::new(5, 6),
            TilePos::new(5, 7),
            TilePos::new(5, 8),
            TilePos::new(6, 8),
        ];
        let land = land_with(&laid);

        let line = line_from(&land, station);
        assert_eq!(line.len(), laid.len());
        assert_eq!(line[0], laid[0], "the front is beside the station");
        assert_eq!(line[3], laid[3], "the back is the far end of the path");
    }

    #[test]
    fn a_station_with_no_queue_path_has_one_place_to_stand() {
        let land = land_with(&[]);
        let line = line_from(&land, TilePos::new(5, 5));

        assert_eq!(line.len(), 1, "an unqueued ride should hold one guest");
        assert!(line[0].neighbours().contains(&TilePos::new(5, 5)));
    }

    #[test]
    fn the_line_never_runs_back_through_the_station() {
        let station = TilePos::new(5, 5);
        // Queue path on both sides of the station: the line should pick one and
        // stay on it rather than crossing over.
        let land = land_with(&[TilePos::new(5, 6), TilePos::new(5, 4)]);

        let line = line_from(&land, station);
        assert_eq!(line.len(), 1, "the line walked through the station");
    }

    #[test]
    fn a_line_is_capped_however_much_path_is_laid() {
        let station = TilePos::new(0, 0);
        #[allow(clippy::cast_possible_wrap)]
        let laid: Vec<TilePos> = (1..60).map(|y| TilePos::new(0, y as i32)).collect();
        let mut land = Land::flat(Grid::filled(4, 64, Terrain::Grass).unwrap()).unwrap();
        for tile in &laid {
            land.set_ground(*tile, Terrain::Queue);
        }

        assert_eq!(line_from(&land, station).len(), Queue::MAX_LENGTH);
    }

    #[test]
    fn a_queue_survives_a_save() {
        let mut queue = Queue::new();
        queue.join(7);
        queue.join(9);

        let json = serde_json::to_string(&queue).unwrap();
        assert_eq!(serde_json::from_str::<Queue>(&json).unwrap(), queue);
    }
}
