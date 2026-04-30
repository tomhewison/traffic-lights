use crate::direction::PairId::{EastWest, NorthSouth};

/// Cardinal direction identifying a traffic light installation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    North,
    South,
    East,
    West,
}

/// Identifies a pair of opposing installations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PairId {
    NorthSouth,
    EastWest,
}

impl Direction {
    /// Returns the pair this direction belongs to.
    pub fn pair_id(self) -> PairId {
        match self {
            Direction::North | Direction::South => NorthSouth,
            Direction::East | Direction::West => EastWest,
        }
    }

    /// Returns the paired (opposing) direction.
    pub fn partner(self) -> Direction {
        match self {
            Direction::North => Direction::South,
            Direction::South => Direction::North,
            Direction::East => Direction::West,
            Direction::West => Direction::East,
        }
    }

    /// Returns true if self and other are on perpendicular roads.
    pub fn intersects(self, other: Direction) -> bool {
        self.pair_id() != other.pair_id() && self != other
    }

    /// Returns true if self and other are in the same pair.
    pub fn is_paired_with(self, other: Direction) -> bool {
        if self.partner() == other && self == other.partner() {
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Pairing — supports I3 (paired synchronisation) and SAF-03
    // =========================================================================

    #[test]
    fn north_is_paired_with_south() {
        assert!(Direction::North.is_paired_with(Direction::South));
    }

    #[test]
    fn south_is_paired_with_north() {
        assert!(Direction::South.is_paired_with(Direction::North));
    }

    #[test]
    fn east_is_paired_with_west() {
        assert!(Direction::East.is_paired_with(Direction::West));
    }

    #[test]
    fn west_is_paired_with_east() {
        assert!(Direction::West.is_paired_with(Direction::East));
    }

    #[test]
    fn north_is_not_paired_with_east() {
        assert!(!Direction::North.is_paired_with(Direction::East));
    }

    #[test]
    fn north_partner_is_south() {
        assert_eq!(Direction::North.partner(), Direction::South);
    }

    #[test]
    fn east_partner_is_west() {
        assert_eq!(Direction::East.partner(), Direction::West);
    }

    // =========================================================================
    // Intersection — supports I1 (mutual exclusion) and SAF-01
    // =========================================================================

    #[test]
    fn north_intersects_east() {
        assert!(Direction::North.intersects(Direction::East));
    }

    #[test]
    fn north_intersects_west() {
        assert!(Direction::North.intersects(Direction::West));
    }

    #[test]
    fn north_does_not_intersect_south() {
        assert!(!Direction::North.intersects(Direction::South));
    }

    #[test]
    fn north_does_not_intersect_itself() {
        assert!(!Direction::North.intersects(Direction::North));
    }

    #[test]
    fn east_intersects_north() {
        assert!(Direction::East.intersects(Direction::North));
    }

    #[test]
    fn east_intersects_south() {
        assert!(Direction::East.intersects(Direction::South));
    }

    #[test]
    fn east_does_not_intersect_west() {
        assert!(!Direction::East.intersects(Direction::West));
    }

    #[test]
    fn south_intersects_east() {
        assert!(Direction::South.intersects(Direction::East));
    }

    #[test]
    fn south_intersects_west() {
        assert!(Direction::South.intersects(Direction::West));
    }

    #[test]
    fn west_intersects_north() {
        assert!(Direction::West.intersects(Direction::North));
    }

    #[test]
    fn west_intersects_south() {
        assert!(Direction::West.intersects(Direction::South));
    }

    #[test]
    fn south_does_not_intersect_north() {
        assert!(!Direction::South.intersects(Direction::North));
    }

    #[test]
    fn west_does_not_intersect_west() {
        assert!(!Direction::West.intersects(Direction::West));
    }

    // =========================================================================
    // Pair IDs
    // =========================================================================

    #[test]
    fn north_pair_id_is_north_south() {
        assert_eq!(Direction::North.pair_id(), PairId::NorthSouth);
    }

    #[test]
    fn south_pair_id_is_north_south() {
        assert_eq!(Direction::South.pair_id(), PairId::NorthSouth);
    }

    #[test]
    fn east_pair_id_is_east_west() {
        assert_eq!(Direction::East.pair_id(), PairId::EastWest);
    }

    #[test]
    fn west_pair_id_is_east_west() {
        assert_eq!(Direction::West.pair_id(), PairId::EastWest);
    }
}
