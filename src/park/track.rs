//! Track: the shape a ride is built out of.
//!
//! A piece of track sits on one tile, faces one of four ways, and leaves the
//! train facing some way at some height. A layout is therefore a starting tile,
//! a starting heading, and a list of pieces — nothing else needs storing,
//! because where every piece ends up follows from the ones before it. Resolving
//! that chain into tiles is [`Track::segments`].
//!
//! There is no geometry here beyond the grid: a curve turns a quarter and takes
//! a tile doing it. Banked turns and half-loops want a spline and a different
//! module, and can have one when the grid stops being enough.

use anyhow::{Context, Result};
use isogrid::iso::TilePos;
use serde::{Deserialize, Serialize};

use crate::park::{Land, Money};

/// One of the four ways a piece of track can face.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Heading {
    /// Towards the top of the map, where the gate is.
    North,
    /// Towards the right.
    East,
    /// Towards the bottom.
    South,
    /// Towards the left.
    West,
}

impl Heading {
    /// Every heading, clockwise from north.
    pub const ALL: [Self; 4] = [Self::North, Self::East, Self::South, Self::West];

    /// A quarter turn clockwise.
    #[must_use]
    pub const fn right(self) -> Self {
        match self {
            Self::North => Self::East,
            Self::East => Self::South,
            Self::South => Self::West,
            Self::West => Self::North,
        }
    }

    /// A quarter turn anticlockwise.
    #[must_use]
    pub const fn left(self) -> Self {
        self.right().right().right()
    }

    /// The way back.
    #[must_use]
    pub const fn about(self) -> Self {
        self.right().right()
    }

    /// The tile one step this way.
    pub fn beyond(self, tile: TilePos) -> TilePos {
        let (dx, dy) = match self {
            Self::North => (0, -1),
            Self::East => (1, 0),
            Self::South => (0, 1),
            Self::West => (-1, 0),
        };
        tile.offset(dx, dy)
    }
}

/// One piece of track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackPiece {
    /// Where trains stop, load and unload. Flat, and the only piece guests can
    /// reach from the ground.
    Station,
    /// Flat and straight.
    Straight,
    /// A quarter turn to the left, taking a tile to do it.
    CurveLeft,
    /// A quarter turn to the right.
    CurveRight,
    /// Up a step, coasting — which a train only manages if it arrives fast
    /// enough.
    SlopeUp,
    /// Down a step. Where the speed comes from.
    SlopeDown,
    /// Up a step under power: the chain lift, which pulls a train up at its own
    /// pace however slowly it arrived.
    LiftHill,
    /// Flat, and slows a train to a crawl. What stops a circuit arriving back at
    /// the station too fast to stop.
    Brakes,
    /// Flat and driven: the train is pushed along at a steady pace whatever it
    /// arrived at.
    ///
    /// What a gentle ride is made of. A coaster earns its speed from its own
    /// height and loses it to friction, so a flat circuit built only of plain
    /// track stalls on its first lap however it is dispatched — powered track is
    /// the honest way to have a ride that does not need a hill.
    Powered,
}

impl TrackPiece {
    /// Every piece that can be built, in the order a toolbar would list them.
    pub const ALL: [Self; 9] = [
        Self::Station,
        Self::Straight,
        Self::CurveLeft,
        Self::CurveRight,
        Self::SlopeUp,
        Self::SlopeDown,
        Self::LiftHill,
        Self::Brakes,
        Self::Powered,
    ];

    /// How many steps this piece climbs: negative for a drop.
    pub const fn climb(self) -> i16 {
        match self {
            Self::SlopeUp | Self::LiftHill => 1,
            Self::SlopeDown => -1,
            Self::Station
            | Self::Straight
            | Self::CurveLeft
            | Self::CurveRight
            | Self::Brakes
            | Self::Powered => 0,
        }
    }

    /// What the train is facing after this piece.
    #[must_use]
    pub const fn steer(self, heading: Heading) -> Heading {
        match self {
            Self::CurveLeft => heading.left(),
            Self::CurveRight => heading.right(),
            Self::Station
            | Self::Straight
            | Self::SlopeUp
            | Self::SlopeDown
            | Self::LiftHill
            | Self::Brakes
            | Self::Powered => heading,
        }
    }

    /// What it costs the park to build one.
    pub const fn cost(self) -> Money {
        match self {
            Self::Station => 350,
            Self::Straight => 60,
            Self::CurveLeft | Self::CurveRight => 80,
            Self::SlopeUp | Self::SlopeDown => 90,
            Self::LiftHill => 140,
            Self::Brakes => 110,
            Self::Powered => 120,
        }
    }

    /// What it costs to keep one piece of track in service for a wage bill.
    pub const fn upkeep(self) -> Money {
        match self {
            Self::Station | Self::LiftHill | Self::Brakes => 3,
            Self::Powered => 4,
            _ => 1,
        }
    }

    /// Whether a train under power on this piece is being pulled rather than
    /// coasting.
    pub const fn is_powered(self) -> bool {
        matches!(self, Self::LiftHill | Self::Station | Self::Powered)
    }

    /// What to call it in the HUD.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Station => "Station",
            Self::Straight => "Straight track",
            Self::CurveLeft => "Curve left",
            Self::CurveRight => "Curve right",
            Self::SlopeUp => "Slope up",
            Self::SlopeDown => "Slope down",
            Self::LiftHill => "Lift hill",
            Self::Brakes => "Brakes",
            Self::Powered => "Powered track",
        }
    }
}

/// One piece of track, resolved to where it actually is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Segment {
    /// Which piece it is.
    pub piece: TrackPiece,
    /// The tile it sits on.
    pub tile: TilePos,
    /// The way a train is facing as it enters.
    pub heading: Heading,
    /// The height a train enters at, in steps.
    pub entry: i16,
    /// The height it leaves at.
    pub exit: i16,
}

/// The whole shape of one ride.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    start: TilePos,
    heading: Heading,
    height: i16,
    pieces: Vec<TrackPiece>,
}

impl Track {
    /// The longest layout worth allowing.
    ///
    /// Long enough for a proper circuit, short enough that resolving the chain
    /// stays cheap at every tick of every train.
    pub const MAX_PIECES: usize = 256;

    /// An empty layout, with its first piece to be laid on `tile` facing
    /// `heading` at `height` steps up.
    pub const fn starting_at(tile: TilePos, heading: Heading, height: i16) -> Self {
        Self {
            start: tile,
            heading,
            height,
            pieces: Vec::new(),
        }
    }

    /// Where the first piece goes.
    pub const fn start(&self) -> TilePos {
        self.start
    }

    /// The way the first piece faces.
    pub const fn heading(&self) -> Heading {
        self.heading
    }

    /// How many pieces have been laid.
    pub fn len(&self) -> usize {
        self.pieces.len()
    }

    /// Whether nothing has been laid yet.
    pub fn is_empty(&self) -> bool {
        self.pieces.is_empty()
    }

    /// The pieces, in the order they were laid.
    pub fn pieces(&self) -> &[TrackPiece] {
        &self.pieces
    }

    /// Every piece, resolved to the tile and height it sits at.
    ///
    /// ```
    /// # use openpark::park::{Heading, Track, TrackPiece};
    /// # use isogrid::iso::TilePos;
    /// let mut track = Track::starting_at(TilePos::new(4, 4), Heading::East, 0);
    /// track.push(TrackPiece::Station)?;
    /// track.push(TrackPiece::LiftHill)?;
    ///
    /// let laid = track.segments();
    /// assert_eq!(laid[0].tile, TilePos::new(4, 4));
    /// assert_eq!(laid[1].tile, TilePos::new(5, 4), "east of the station");
    /// assert_eq!(laid[1].exit, 1, "and a step higher than it started");
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn segments(&self) -> Vec<Segment> {
        let mut laid = Vec::with_capacity(self.pieces.len());
        let mut tile = self.start;
        let mut heading = self.heading;
        let mut height = self.height;

        for piece in &self.pieces {
            let exit = height + piece.climb();
            laid.push(Segment {
                piece: *piece,
                tile,
                heading,
                entry: height,
                exit,
            });

            tile = piece.steer(heading).beyond(tile);
            heading = piece.steer(heading);
            height = exit;
        }

        laid
    }

    /// Where the next piece would go, and facing which way, at what height.
    pub fn next_place(&self) -> (TilePos, Heading, i16) {
        match self.segments().last() {
            None => (self.start, self.heading, self.height),
            Some(last) => {
                let heading = last.piece.steer(last.heading);
                (heading.beyond(last.tile), heading, last.exit)
            }
        }
    }

    /// Lays one more piece, returning where it went.
    ///
    /// # Errors
    ///
    /// Fails if the layout is already [`Track::MAX_PIECES`] long, if the piece
    /// would leave the height range the land allows, or if it would land on a
    /// tile this layout already occupies — track that crosses over itself needs
    /// a supporting structure this does not model yet.
    pub fn push(&mut self, piece: TrackPiece) -> Result<Segment> {
        anyhow::ensure!(
            self.pieces.len() < Self::MAX_PIECES,
            "a layout cannot be longer than {} pieces",
            Self::MAX_PIECES,
        );

        let (tile, _, entry) = self.next_place();
        let exit = entry + piece.climb();
        anyhow::ensure!(
            (Land::MIN_HEIGHT..=Land::MAX_HEIGHT).contains(&exit),
            "track cannot run above {} steps or below {}",
            Land::MAX_HEIGHT,
            Land::MIN_HEIGHT,
        );
        anyhow::ensure!(
            !self.occupies(tile),
            "this ride already runs across {tile:?}",
        );

        self.pieces.push(piece);
        self.segments()
            .last()
            .copied()
            .context("a piece was laid and then lost")
    }

    /// Takes the last piece back off, returning it.
    pub fn pop(&mut self) -> Option<TrackPiece> {
        self.pieces.pop()
    }

    /// Whether any piece of this layout sits on `tile`.
    pub fn occupies(&self, tile: TilePos) -> bool {
        self.segments().iter().any(|laid| laid.tile == tile)
    }

    /// Every tile this layout stands on.
    pub fn tiles(&self) -> Vec<TilePos> {
        self.segments().iter().map(|laid| laid.tile).collect()
    }

    /// The station tiles, which are the only ones guests can reach.
    pub fn stations(&self) -> Vec<TilePos> {
        self.segments()
            .iter()
            .filter(|laid| laid.piece == TrackPiece::Station)
            .map(|laid| laid.tile)
            .collect()
    }

    /// Whether the track comes back to where it started, facing the same way at
    /// the same height.
    ///
    /// A ride has to be a circuit to run: a train that reaches the end of an
    /// unfinished layout has nowhere to be.
    ///
    /// ```
    /// # use openpark::park::{Heading, Track, TrackPiece};
    /// # use isogrid::iso::TilePos;
    /// let mut track = Track::starting_at(TilePos::new(4, 4), Heading::East, 0);
    /// for _ in 0..3 {
    ///     track.push(TrackPiece::CurveRight)?;
    /// }
    /// assert!(!track.is_a_circuit(), "three sides of a square are not a lap");
    ///
    /// track.push(TrackPiece::CurveRight)?;
    /// assert!(track.is_a_circuit(), "four right turns come back round");
    /// # Ok::<(), anyhow::Error>(())
    /// ```
    pub fn is_a_circuit(&self) -> bool {
        if self.pieces.len() < 4 {
            return false;
        }

        let (tile, heading, height) = self.next_place();
        tile == self.start && heading == self.heading && height == self.height
    }

    /// What the whole layout cost to build.
    pub fn cost(&self) -> Money {
        self.pieces.iter().map(|piece| piece.cost()).sum()
    }

    /// What the whole layout costs to keep in service for one wage bill.
    pub fn upkeep(&self) -> Money {
        self.pieces.iter().map(|piece| piece.upkeep()).sum()
    }

    /// The highest and lowest a train gets, in steps.
    pub fn range(&self) -> (i16, i16) {
        let heights = self.segments();
        let lowest = heights
            .iter()
            .map(|laid| laid.entry.min(laid.exit))
            .min()
            .unwrap_or(self.height);
        let highest = heights
            .iter()
            .map(|laid| laid.entry.max(laid.exit))
            .max()
            .unwrap_or(self.height);

        (lowest, highest)
    }

    /// The biggest single run of falling track, in steps.
    ///
    /// The drop is what a coaster is judged on, so it is measured as the longest
    /// unbroken descent rather than the total.
    pub fn longest_drop(&self) -> i16 {
        let mut longest = 0;
        let mut falling = 0;

        for laid in self.segments() {
            if laid.piece == TrackPiece::SlopeDown {
                falling += 1;
                longest = longest.max(falling);
            } else {
                falling = 0;
            }
        }

        longest
    }

    /// How many quarter turns the layout takes.
    pub fn curves(&self) -> usize {
        self.pieces
            .iter()
            .filter(|piece| matches!(piece, TrackPiece::CurveLeft | TrackPiece::CurveRight))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A three-by-two ring: four corners to turn on, the station along one
    /// side, and one straight on the other. The smallest circuit with somewhere
    /// for guests to get on.
    ///
    /// A corner has to turn, so the station cannot sit on one — which is why
    /// the layout starts on the corner before it, facing north.
    fn oval() -> Track {
        let mut track = Track::starting_at(TilePos::new(8, 8), Heading::North, 0);
        for piece in [
            TrackPiece::CurveRight,
            TrackPiece::Station,
            TrackPiece::CurveRight,
            TrackPiece::CurveRight,
            TrackPiece::Straight,
            TrackPiece::CurveRight,
        ] {
            track.push(piece).expect("the oval should lay");
        }
        track
    }

    #[test]
    fn every_heading_comes_back_round() {
        for heading in Heading::ALL {
            assert_eq!(heading.right().right().right().right(), heading);
            assert_eq!(heading.left(), heading.right().right().right());
            assert_eq!(heading.about(), heading.right().right());
            assert_ne!(heading.about(), heading);
        }
    }

    #[test]
    fn a_heading_walks_the_way_it_points() {
        let tile = TilePos::new(5, 5);
        assert_eq!(Heading::North.beyond(tile), TilePos::new(5, 4));
        assert_eq!(Heading::South.beyond(tile), TilePos::new(5, 6));
        assert_eq!(Heading::East.beyond(tile), TilePos::new(6, 5));
        assert_eq!(Heading::West.beyond(tile), TilePos::new(4, 5));
    }

    #[test]
    fn an_empty_layout_has_nothing_in_it() {
        let track = Track::starting_at(TilePos::ORIGIN, Heading::North, 0);
        assert!(track.is_empty());
        assert_eq!(track.len(), 0);
        assert_eq!(track.cost(), 0);
        assert!(track.segments().is_empty());
        assert!(!track.is_a_circuit(), "nothing is not a circuit");
        assert_eq!(track.next_place(), (TilePos::ORIGIN, Heading::North, 0));
    }

    #[test]
    fn pieces_follow_one_another_along_the_grid() {
        let track = oval();
        let laid = track.segments();

        assert_eq!(laid.len(), 6);
        assert_eq!(laid[0].tile, TilePos::new(8, 8), "the first corner");
        assert_eq!(laid[1].tile, TilePos::new(9, 8), "the station, facing east");
        assert_eq!(laid[1].heading, Heading::East);
        assert_eq!(laid[2].tile, TilePos::new(10, 8));
        assert_eq!(laid[3].tile, TilePos::new(10, 9), "turned south");
        assert_eq!(laid[3].heading, Heading::South);
    }

    #[test]
    fn four_right_turns_make_a_circuit() {
        let track = oval();
        assert!(track.is_a_circuit(), "the oval does not close");

        let mut open = oval();
        open.pop();
        assert!(!open.is_a_circuit(), "it closed with a piece missing");
    }

    #[test]
    fn a_layout_cannot_cross_itself() {
        let mut track = Track::starting_at(TilePos::new(8, 8), Heading::East, 0);
        for piece in [
            TrackPiece::Station,
            TrackPiece::CurveRight,
            TrackPiece::CurveRight,
            TrackPiece::CurveRight,
        ] {
            track.push(piece).unwrap();
        }

        // The fourth turn would land back on the station.
        assert!(track.push(TrackPiece::CurveRight).is_err());
        assert_eq!(track.len(), 4, "the refused piece was laid anyway");
    }

    #[test]
    fn track_cannot_climb_out_of_the_park() {
        let mut track = Track::starting_at(TilePos::new(0, 8), Heading::East, 0);
        track.push(TrackPiece::Station).unwrap();
        for _ in 0..Land::MAX_HEIGHT {
            track.push(TrackPiece::LiftHill).unwrap();
        }

        assert!(track.push(TrackPiece::LiftHill).is_err(), "into orbit");
        assert!(
            track.push(TrackPiece::SlopeDown).is_ok(),
            "but coming back down is fine"
        );
    }

    #[test]
    fn track_cannot_dig_below_the_base() {
        let mut track = Track::starting_at(TilePos::new(0, 8), Heading::East, 0);
        track.push(TrackPiece::Station).unwrap();
        assert!(track.push(TrackPiece::SlopeDown).is_err());
    }

    #[test]
    fn a_layout_is_capped_in_length() {
        let mut track = Track::starting_at(TilePos::ORIGIN, Heading::East, 0);
        // Turning in a big spiral would run out of tiles; a straight line is
        // the cheapest way to lay a great many pieces.
        for _ in 0..Track::MAX_PIECES {
            track.push(TrackPiece::Straight).unwrap();
        }

        assert!(track.push(TrackPiece::Straight).is_err());
        assert_eq!(track.len(), Track::MAX_PIECES);
    }

    #[test]
    fn the_station_is_the_only_way_in() {
        let track = oval();
        assert_eq!(track.stations(), vec![TilePos::new(9, 8)]);
        assert_eq!(track.tiles().len(), track.len());
    }

    #[test]
    fn what_a_layout_costs_is_what_its_pieces_cost() {
        let track = oval();
        let by_hand: Money = track.pieces().iter().map(|piece| piece.cost()).sum();

        assert_eq!(track.cost(), by_hand);
        assert!(track.cost() > 0);
        assert!(track.upkeep() > 0);
    }

    #[test]
    fn the_range_and_the_drop_are_measured_off_the_layout() {
        let mut track = Track::starting_at(TilePos::new(8, 8), Heading::East, 0);
        for piece in [
            TrackPiece::Station,
            TrackPiece::LiftHill,
            TrackPiece::LiftHill,
            TrackPiece::SlopeDown,
            TrackPiece::SlopeDown,
            TrackPiece::Straight,
            TrackPiece::SlopeUp,
        ] {
            track.push(piece).unwrap();
        }

        assert_eq!(track.range(), (0, 2));
        assert_eq!(track.longest_drop(), 2, "two falling pieces in a row");
        assert_eq!(track.curves(), 0);
    }

    #[test]
    fn the_drop_is_the_longest_run_not_the_total() {
        let mut track = Track::starting_at(TilePos::new(8, 8), Heading::East, 0);
        for piece in [
            TrackPiece::Station,
            TrackPiece::LiftHill,
            TrackPiece::LiftHill,
            TrackPiece::LiftHill,
            TrackPiece::SlopeDown,
            TrackPiece::Straight,
            TrackPiece::SlopeDown,
            TrackPiece::SlopeDown,
        ] {
            track.push(piece).unwrap();
        }

        assert_eq!(track.longest_drop(), 2);
    }

    #[test]
    fn every_piece_knows_what_it_does() {
        for piece in TrackPiece::ALL {
            assert!(piece.cost() > 0, "{piece:?} is free");
            assert!(piece.upkeep() > 0, "{piece:?} costs nothing to run");
            assert!(!piece.name().is_empty());
            assert!((-1..=1).contains(&piece.climb()), "{piece:?} climbs oddly");
        }

        assert_eq!(TrackPiece::CurveLeft.steer(Heading::North), Heading::West);
        assert_eq!(TrackPiece::CurveRight.steer(Heading::North), Heading::East);
        assert_eq!(TrackPiece::Straight.steer(Heading::North), Heading::North);
        assert!(TrackPiece::LiftHill.is_powered());
        assert!(!TrackPiece::SlopeDown.is_powered());
    }

    #[test]
    fn a_layout_survives_a_save() {
        let track = oval();
        let json = serde_json::to_string(&track).unwrap();
        assert_eq!(serde_json::from_str::<Track>(&json).unwrap(), track);
    }
}
