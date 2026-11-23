use crate::{Direction, Point};

/// The same cubic command stream can be drawn by native Apple and Windows renderers.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// Begin a new subpath at this location without drawing a segment.
    Move(Point),
    /// Draw a line from the current location to this point.
    Line(Point),
    /// Draw a cubic Bézier segment from the current location.
    Cubic {
        /// First control point.
        control1: Point,
        /// Second control point.
        control2: Point,
        /// Segment endpoint.
        to: Point,
    },
    /// Close the current subpath to its initial location.
    Close,
}

/// Owned drawing commands, independent of input buffers and native graphics APIs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Path {
    /// Commands in drawing order.
    pub commands: Vec<Command>,
}

/// Planned shaft and optional arrowhead for a direction-pattern preview.
#[derive(Debug, Clone, PartialEq)]
pub struct Geometry {
    /// Stroke path, capped before the arrowhead tip when a head fits.
    pub path: Path,
    /// Closed arrowhead path to fill, absent when disabled or too small to fit.
    pub arrowhead: Option<Path>,
}

/// Rounding for live traces; preview layout uses its own legible corner geometry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CornerStyle {
    /// Multiplies the 24-point maximum live corner radius; zero keeps sharp turns.
    pub radius_scale: f64,
}

impl Default for CornerStyle {
    fn default() -> Self {
        Self { radius_scale: 1.0 }
    }
}

/// Invalid trace input or geometry that cannot be represented with finite coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum GeometryError {
    /// No cardinal directions were supplied.
    Empty = 1,
    /// Point count is not exactly one more than direction count.
    PointCount = 2,
    /// Input or computed coordinates are non-finite.
    NonFinite = 3,
    /// A foreign input buffer is invalid; reported by the native boundary.
    InvalidBuffer = 4,
    /// A foreign numeric direction is invalid; reported by the native boundary.
    InvalidDirection = 5,
}

impl std::fmt::Display for GeometryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Empty => "a gesture requires at least one direction",
            Self::PointCount => "points must contain one more element than directions",
            Self::NonFinite => "coordinates or computed geometry are not finite",
            Self::InvalidBuffer => "invalid native input buffer",
            Self::InvalidDirection => "invalid native direction code",
        })
    }
}
impl std::error::Error for GeometryError {}

/// Replace non-finite drawing inputs with zero, preserving finite signed values.
pub fn finite_or_zero(value: f64) -> f64 {
    if value.is_finite() { value } else { 0.0 }
}
/// Normalize a drawing width, padding or corner scale.
pub fn non_negative(value: f64) -> f64 {
    finite_or_zero(value).max(0.0)
}
pub(crate) fn lerp(a: Point, b: Point, t: f64) -> Point {
    Point {
        x: a.x * (1.0 - t) + b.x * t,
        y: a.y * (1.0 - t) + b.y * t,
    }
}
pub(crate) fn vector(d: Direction) -> Point {
    match d {
        Direction::Up => Point { x: 0.0, y: -1.0 },
        Direction::Down => Point { x: 0.0, y: 1.0 },
        Direction::Left => Point { x: -1.0, y: 0.0 },
        Direction::Right => Point { x: 1.0, y: 0.0 },
    }
}
pub(crate) fn offset(p: Point, v: Point, distance: f64) -> Point {
    Point {
        x: p.x + v.x * distance,
        y: p.y + v.y * distance,
    }
}

pub(crate) fn validate_trace_shape(
    directions: &[Direction],
    points: &[Point],
    tail: Option<Point>,
) -> Result<(), GeometryError> {
    if directions.is_empty() {
        return Err(GeometryError::Empty);
    }
    if directions.len().checked_add(1) != Some(points.len()) {
        return Err(GeometryError::PointCount);
    }
    if tail.is_some_and(|point| !point.is_finite()) {
        return Err(GeometryError::NonFinite);
    }
    Ok(())
}

/// Project each captured run onto its cardinal axis while retaining the stroke
/// origin. Round perpendicular corners once, bounded by 24 points at the default
/// scale and half of either adjacent segment, matching the website trace style.
/// Non-finite points or unrepresentable projected segment lengths return an error.
pub fn live_trace(
    directions: &[Direction],
    points: &[Point],
    tail: Option<Point>,
    style: CornerStyle,
) -> Result<Path, GeometryError> {
    validate_trace_shape(directions, points, tail)?;
    if points.iter().any(|point| !point.is_finite()) {
        return Err(GeometryError::NonFinite);
    }
    let point_at = |index| {
        if index + 1 == points.len() {
            tail.unwrap_or(points[index])
        } else {
            points[index]
        }
    };
    let horizontal = |direction| matches!(direction, Direction::Left | Direction::Right);
    let mut projected = vec![points[0]];
    for (index, &direction) in directions.iter().enumerate() {
        let target = if directions
            .get(index + 1)
            .is_some_and(|&next| horizontal(next) != horizontal(direction))
        {
            point_at(index + 2)
        } else {
            point_at(index + 1)
        };
        let previous = *projected.last().unwrap();
        projected.push(if horizontal(direction) {
            Point {
                x: target.x,
                y: previous.y,
            }
        } else {
            Point {
                x: previous.x,
                y: target.y,
            }
        });
    }
    // Validate the segments that will be drawn, including a substituted tail.
    // Projection makes each segment axis-aligned, so only one delta can be nonzero.
    if projected
        .windows(2)
        .any(|pair| !(pair[1].x - pair[0].x).is_finite() || !(pair[1].y - pair[0].y).is_finite())
    {
        return Err(GeometryError::NonFinite);
    }
    let radius = (24.0 * non_negative(style.radius_scale)).min(f64::MAX);
    rounded_path(&projected, radius)
}

pub(crate) fn rounded_path(points: &[Point], radius: f64) -> Result<Path, GeometryError> {
    let Some(&first) = points.first() else {
        return Err(GeometryError::Empty);
    };
    let radius = non_negative(radius);
    let mut commands = vec![Command::Move(first)];
    for triple in points.windows(3) {
        let [a, b, c] = [triple[0], triple[1], triple[2]];
        let incoming = Point {
            x: b.x - a.x,
            y: b.y - a.y,
        };
        let outgoing = Point {
            x: c.x - b.x,
            y: c.y - b.y,
        };
        let before = incoming.x.hypot(incoming.y);
        let after = outgoing.x.hypot(outgoing.y);
        if !before.is_finite() || !after.is_finite() {
            return Err(GeometryError::NonFinite);
        }
        if before == 0.0 || after == 0.0 || radius == 0.0 {
            commands.push(Command::Line(b));
            continue;
        }
        let cross = (incoming.x / before) * (outgoing.y / after)
            - (incoming.y / before) * (outgoing.x / after);
        if cross.abs() < 1e-12 {
            commands.push(Command::Line(b));
            continue;
        }
        let limit = before.min(after) * 0.5;
        let cut = radius.min(limit);
        if cut == 0.0 {
            commands.push(Command::Line(b));
            continue;
        }
        let start = lerp(b, a, cut / before);
        let end = lerp(b, c, cut / after);
        commands.push(Command::Line(start));
        commands.push(Command::Cubic {
            control1: lerp(start, b, 2.0 / 3.0),
            control2: lerp(end, b, 2.0 / 3.0),
            to: end,
        });
    }
    if points.len() > 1 {
        commands.push(Command::Line(*points.last().unwrap()));
    }
    let path = Path { commands };
    if !path.is_finite() {
        return Err(GeometryError::NonFinite);
    }
    Ok(path)
}

impl Path {
    /// Whether every endpoint and cubic control point is finite.
    pub fn is_finite(&self) -> bool {
        self.commands.iter().all(|c| match c {
            Command::Move(p) | Command::Line(p) => p.is_finite(),
            Command::Cubic {
                control1,
                control2,
                to,
            } => control1.is_finite() && control2.is_finite() && to.is_finite(),
            Command::Close => true,
        })
    }
    pub(crate) fn transform(&mut self, scale: f64, origin: Point) {
        fn apply(p: &mut Point, scale: f64, origin: Point) {
            p.x = origin.x + p.x * scale;
            p.y = origin.y + p.y * scale;
        }
        for c in &mut self.commands {
            match c {
                Command::Move(p) | Command::Line(p) => apply(p, scale, origin),
                Command::Cubic {
                    control1,
                    control2,
                    to,
                } => {
                    apply(control1, scale, origin);
                    apply(control2, scale, origin);
                    apply(to, scale, origin);
                }
                Command::Close => {}
            }
        }
    }
}

/// Conservative bounds include curve controls; they are not pixel hit tests.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Bounds {
    /// Left edge in the caller's coordinate space.
    pub min_x: f64,
    /// Minimum y coordinate.
    pub min_y: f64,
    /// Right edge in the caller's coordinate space.
    pub max_x: f64,
    /// Maximum y coordinate.
    pub max_y: f64,
}
impl Bounds {
    /// Enclose finite points; empty or non-finite input returns none.
    pub fn from_points(points: &[Point]) -> Option<Self> {
        let (&first, remaining) = points.split_first()?;
        if !first.is_finite() {
            return None;
        }
        let mut value = Self {
            min_x: first.x,
            min_y: first.y,
            max_x: first.x,
            max_y: first.y,
        };
        for &point in remaining {
            if !point.is_finite() {
                return None;
            }
            value.include(point);
        }
        Some(value)
    }
    pub(crate) fn include(&mut self, p: Point) {
        self.min_x = self.min_x.min(p.x);
        self.min_y = self.min_y.min(p.y);
        self.max_x = self.max_x.max(p.x);
        self.max_y = self.max_y.max(p.y);
    }
    /// Test conservative visibility, including the optional tail and half the
    /// nonnegative line width. Edge contact counts. Invalid or inverted bounds
    /// and non-finite tails return false; all values must share one coordinate space.
    pub fn intersects(mut self, tail: Option<Point>, viewport: Self, line_width: f64) -> bool {
        if ![
            self.min_x,
            self.min_y,
            self.max_x,
            self.max_y,
            viewport.min_x,
            viewport.min_y,
            viewport.max_x,
            viewport.max_y,
        ]
        .iter()
        .all(|x| x.is_finite())
            || self.max_x < self.min_x
            || self.max_y < self.min_y
            || viewport.max_x < viewport.min_x
            || viewport.max_y < viewport.min_y
        {
            return false;
        }
        if let Some(tail) = tail {
            if !tail.is_finite() {
                return false;
            }
            self.include(tail);
        }
        let padding = non_negative(line_width) / 2.0;
        self.max_x >= viewport.min_x - padding
            && viewport.max_x + padding >= self.min_x
            && self.max_y >= viewport.min_y - padding
            && viewport.max_y + padding >= self.min_y
    }
}
