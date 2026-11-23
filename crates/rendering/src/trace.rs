use crate::{Bounds, Direction, GeometryError, Point};
use std::sync::Arc;

/// An immutable, validated trace. Clones share coordinates and cached bounds.
#[derive(Debug, Clone)]
pub struct OverlayTrace {
    directions: Arc<[Direction]>,
    points: Arc<[Point]>,
    tail: Option<Point>,
    bounds: Bounds,
}
impl PartialEq for OverlayTrace {
    fn eq(&self, other: &Self) -> bool {
        // Validation excludes NaN and storage is immutable, so shared identity is
        // a valid equality shortcut. Check the changing tail before stable arrays.
        self.tail == other.tail
            && (Arc::ptr_eq(&self.directions, &other.directions)
                || self.directions == other.directions)
            && (Arc::ptr_eq(&self.points, &other.points) || self.points == other.points)
    }
}
impl OverlayTrace {
    /// Copy a nonempty direction sequence, one more finite point than directions,
    /// and an optional finite tail. Invalid input returns [`GeometryError`].
    pub fn new(
        directions: &[Direction],
        points: &[Point],
        tail: Option<Point>,
    ) -> Result<Self, GeometryError> {
        if directions.is_empty() {
            return Err(GeometryError::Empty);
        }
        if directions.len().checked_add(1) != Some(points.len()) {
            return Err(GeometryError::PointCount);
        }
        if !points.iter().all(|p| p.is_finite()) || tail.is_some_and(|p| !p.is_finite()) {
            return Err(GeometryError::NonFinite);
        }
        Ok(Self {
            directions: directions.into(),
            points: points.into(),
            tail,
            bounds: Bounds::from_points(points).ok_or(GeometryError::NonFinite)?,
        })
    }
    /// Convert a recognition snapshot into an owned rendering trace.
    ///
    /// Hidden snapshots return `Ok(None)` without using their geometry.
    /// Visible snapshots reuse a previous trace's geometry when its directions
    /// and points match. The returned flag reports that reuse to bindings.
    /// An invalid visible snapshot returns a geometry error.
    pub fn from_snapshot(
        previous: Option<&Self>,
        visible: bool,
        directions: &[Direction],
        points: &[Point],
        tail: Option<Point>,
    ) -> Result<Option<(Self, bool)>, GeometryError> {
        if !visible {
            return Ok(None);
        }
        if let Some(previous) = previous
            && previous.directions() == directions
            && previous.points() == points
        {
            return previous.with_tail(tail).map(|trace| Some((trace, true)));
        }
        Self::new(directions, points, tail).map(|trace| Some((trace, false)))
    }

    /// Return a new immutable trace sharing coordinates and cached bounds.
    /// Only the optional tail is replaced; non-finite tails leave this trace intact.
    pub fn with_tail(&self, tail: Option<Point>) -> Result<Self, GeometryError> {
        if tail.is_some_and(|p| !p.is_finite()) {
            return Err(GeometryError::NonFinite);
        }
        Ok(Self {
            tail,
            ..self.clone()
        })
    }
    /// Cardinal runs in drawing order, borrowing this trace's shared storage.
    pub fn directions(&self) -> &[Direction] {
        &self.directions
    }
    /// Stroke origin followed by one endpoint per direction, borrowing this trace.
    pub fn points(&self) -> &[Point] {
        &self.points
    }
    /// Latest pointer location, when provided separately from captured endpoints.
    pub fn tail(&self) -> Option<Point> {
        self.tail
    }
    /// Conservative cached-bounds visibility including tail and stroke padding.
    /// See [`Bounds::intersects`] for invalid inputs and edge-contact behavior.
    pub fn intersects(&self, viewport: Bounds, line_width: f64) -> bool {
        self.bounds.intersects(self.tail, viewport, line_width)
    }
    pub(crate) fn leading_point(&self) -> Point {
        self.tail
            .unwrap_or(*self.points.last().expect("validated trace"))
    }
}
