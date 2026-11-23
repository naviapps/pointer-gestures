use std::cmp::Ordering;

/// A pointer location. Coordinates increase rightward and downward.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Point {
    /// Horizontal coordinate, increasing rightward.
    pub x: f64,
    /// Vertical coordinate, increasing downward.
    pub y: f64,
}
impl Point {
    /// Whether both coordinates are finite.
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

/// Cardinal direction with stable codes shared by the C and managed adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Direction {
    /// Decreasing y.
    Up = 1,
    /// Increasing y.
    Down = 2,
    /// Decreasing x.
    Left = 3,
    /// Increasing x.
    Right = 4,
}
impl TryFrom<u32> for Direction {
    type Error = InvalidDirection;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Up),
            2 => Ok(Self::Down),
            3 => Ok(Self::Left),
            4 => Ok(Self::Right),
            _ => Err(InvalidDirection),
        }
    }
}
/// A numeric direction code outside the four cardinal directions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidDirection;
impl std::fmt::Display for InvalidDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("invalid cardinal direction code")
    }
}
impl std::error::Error for InvalidDirection {}

/// Dominant-axis movement between two finite locations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Movement {
    /// Dominant cardinal direction, or none for equal absolute axis deltas.
    pub direction: Option<Direction>,
    /// Larger absolute axis delta; extreme finite endpoints can overflow to infinity.
    pub maximum_axis_delta: f64,
}
/// Rejects non-finite coordinates. Equal axis deltas have no dominant direction.
pub fn resolve_movement(start: Point, end: Point) -> Option<Movement> {
    if !start.is_finite() || !end.is_finite() {
        return None;
    }
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let ax = dx.abs();
    let ay = dy.abs();
    let dominant = if ax.is_infinite() && ay.is_infinite() {
        half_delta_sum(start.x, end.x).partial_cmp(&half_delta_sum(start.y, end.y))
    } else {
        ax.partial_cmp(&ay)
    };
    let direction = match dominant {
        Some(Ordering::Greater) => Some(if dx >= 0.0 {
            Direction::Right
        } else {
            Direction::Left
        }),
        Some(Ordering::Less) => Some(if dy >= 0.0 {
            Direction::Down
        } else {
            Direction::Up
        }),
        _ => None,
    };
    Some(Movement {
        direction,
        maximum_axis_delta: ax.max(ay),
    })
}

// An overflowing delta between finite endpoints has opposite signs. Halve
// their magnitudes to fit the sum, and keep its rounding error to resolve ties.
fn half_delta_sum(start: f64, end: f64) -> (f64, f64) {
    let start = start.abs() * 0.5;
    let end = end.abs() * 0.5;
    let (large, small) = (start.max(end), start.min(end));
    let sum = large + small;
    (sum, small - (sum - large))
}
