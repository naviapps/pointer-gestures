use crate::path::{non_negative, offset, rounded_path, vector};
use crate::{Bounds, Command, Direction, Geometry, GeometryError, Path, Point};
use std::collections::HashSet;

/// Fits a direction preview and its arrowhead to one canvas. Reversals receive a
/// small perpendicular detour so their strokes remain distinguishable.
/// Dimensions, padding and stroke width use one coordinate unit; negative or
/// non-finite scalars become zero. Empty input returns [`GeometryError::Empty`].
/// Disabling the arrowhead preserves the shaft's layout; a head too large for the
/// final run is omitted. Corner scale multiplies a radius of one tenth of the
/// smaller canvas dimension; zero keeps sharp turns. Unrepresentable computed
/// coordinates return an error. Arrowheads scale with the canvas and stroke;
/// same-axis reversals retain one detour side until a perpendicular turn.
pub fn pattern(
    directions: &[Direction],
    width: f64,
    height: f64,
    padding: f64,
    line_width: f64,
    shows_arrowhead: bool,
    corner_radius_scale: f64,
) -> Result<Geometry, GeometryError> {
    if directions.is_empty() {
        return Err(GeometryError::Empty);
    }
    let width = non_negative(width);
    let height = non_negative(height);
    let line_width = non_negative(line_width);
    let padding = non_negative(padding).max(line_width * 0.5);
    let available_w = (width - padding * 2.0).max(0.0);
    let available_h = (height - padding * 2.0).max(0.0);
    if available_w <= line_width || available_h <= line_width {
        let p = Point {
            x: width / 2.0,
            y: height / 2.0,
        };
        return Ok(Geometry {
            path: Path {
                commands: vec![Command::Move(p), Command::Line(p)],
            },
            arrowhead: None,
        });
    }
    let mut points = vec![Point { x: 0.0, y: 0.0 }];
    let mut current = points[0];
    let mut visited = HashSet::from([(0_i64, 0_i64)]);
    let mut reversal_side = None;
    for (i, &direction) in directions.iter().enumerate() {
        let next = vector(direction);
        if i > 0 {
            let previous = vector(directions[i - 1]);
            if previous.x == -next.x && previous.y == -next.y {
                let side = *reversal_side.get_or_insert(Point {
                    x: -previous.y,
                    y: previous.x,
                });
                points.push(offset(current, previous, 0.25));
                current = offset(*points.last().unwrap(), side, 0.4);
                points.push(current);
                current = offset(current, next, 0.25);
                points.push(current);
            } else if previous.x * next.x + previous.y * next.y == 0.0 {
                reversal_side = None;
            }
        }
        current = offset(current, next, 1.0);
        let key = (
            (current.x * 20.0).round() as i64,
            (current.y * 20.0).round() as i64,
        );
        // Leave an already visited vertex slightly before it so the next stroke
        // exits beside the earlier stroke instead of hiding underneath it.
        if visited.contains(&key) && i + 1 < directions.len() {
            current = offset(current, next, -0.4);
        }
        if i + 1 == directions.len()
            && (reversal_side.is_some()
                || points.windows(2).any(|edge| {
                    let [a, b] = [edge[0], edge[1]];
                    (a.x == b.x
                        && (current.x - a.x).abs() < 1e-9
                        && current.y >= a.y.min(b.y)
                        && current.y <= a.y.max(b.y))
                        || (a.y == b.y
                            && (current.y - a.y).abs() < 1e-9
                            && current.x >= a.x.min(b.x)
                            && current.x <= a.x.max(b.x))
                }))
        {
            // A final arrow must leave the earlier stroke or reversal lanes.
            // Distance is illustrative; extending the final run preserves directions.
            let bounds = Bounds::from_points(&points).ok_or(GeometryError::NonFinite)?;
            current = match direction {
                Direction::Right => Point {
                    x: current.x.max(bounds.max_x + 0.35),
                    y: current.y,
                },
                Direction::Left => Point {
                    x: current.x.min(bounds.min_x - 0.35),
                    y: current.y,
                },
                Direction::Down => Point {
                    x: current.x,
                    y: current.y.max(bounds.max_y + 0.35),
                },
                Direction::Up => Point {
                    x: current.x,
                    y: current.y.min(bounds.min_y - 0.35),
                },
            };
        }
        visited.insert((
            (current.x * 20.0).round() as i64,
            (current.y * 20.0).round() as i64,
        ));
        points.push(current);
    }
    let last = vector(*directions.last().unwrap());
    let normal = Point {
        x: -last.y,
        y: last.x,
    };
    let bounds = Bounds::from_points(&points).ok_or(GeometryError::NonFinite)?;
    let maximum_scale = (available_w / (bounds.max_x - bounds.min_x))
        .min(available_h / (bounds.max_y - bounds.min_y));
    if !maximum_scale.is_finite() || maximum_scale <= 0.0 {
        return Err(GeometryError::NonFinite);
    }
    let final_start = points[points.len() - 2];
    let final_length = (current.x - final_start.x).hypot(current.y - final_start.y);
    // Include the real, stroke-sized head in the fit. This is constant work per
    // trial: only the polyline bounds and three arrow vertices are needed.
    let arrow_at = |scale: f64| {
        let size = scale
            .min(width.min(height) * 0.3)
            .max((line_width * 3.0).min(f64::MAX));
        let end = Point {
            x: current.x * scale,
            y: current.y * scale,
        };
        let back = offset(end, last, -0.3 * size);
        (
            size,
            [
                offset(back, normal, 0.25 * size),
                offset(back, normal, -0.25 * size),
                offset(end, last, 0.15 * size),
            ],
        )
    };
    // Fit the painted shaft, including round caps, and the filled head together.
    // The last centerline point is shortened to the head base before its bounds
    // are measured; counting the original endpoint shifts thick arrows off center.
    let shaft_bounds =
        Bounds::from_points(&points[..points.len() - 1]).ok_or(GeometryError::NonFinite)?;
    let fitted_bounds = |scale: f64, head: Option<(f64, [Point; 3])>| {
        let mut fitted = Bounds {
            min_x: shaft_bounds.min_x * scale,
            min_y: shaft_bounds.min_y * scale,
            max_x: shaft_bounds.max_x * scale,
            max_y: shaft_bounds.max_y * scale,
        };
        let end = Point {
            x: current.x * scale,
            y: current.y * scale,
        };
        fitted.include(head.map_or(end, |(size, _)| offset(end, last, -0.3 * size)));
        fitted.min_x -= line_width * 0.5;
        fitted.min_y -= line_width * 0.5;
        fitted.max_x += line_width * 0.5;
        fitted.max_y += line_width * 0.5;
        if let Some((_, arrow)) = head {
            for point in arrow {
                fitted.include(point);
            }
        }
        fitted
    };
    let fits = |bounds: Bounds| {
        bounds.max_x - bounds.min_x <= available_w && bounds.max_y - bounds.min_y <= available_h
    };
    let mut scale = ((available_w - line_width) / (bounds.max_x - bounds.min_x))
        .min((available_h - line_width) / (bounds.max_y - bounds.min_y));
    let mut head = None;
    let (size, _) = arrow_at(maximum_scale);
    if final_length * maximum_scale >= size * 0.6 && fits(fitted_bounds(0.0, Some(arrow_at(0.0)))) {
        let mut lower = 0.0;
        let mut upper = maximum_scale;
        for _ in 0..48 {
            let middle = lower + (upper - lower) * 0.5;
            if fits(fitted_bounds(middle, Some(arrow_at(middle)))) {
                lower = middle;
            } else {
                upper = middle;
            }
        }
        let (size, arrow) = arrow_at(lower);
        if final_length * lower >= size * 0.6 {
            scale = lower;
            head = Some((size, arrow));
        }
    }
    if head.is_some_and(|(_, a)| a[0] == a[1] || a[0] == a[2] || a[1] == a[2]) {
        return Err(GeometryError::NonFinite);
    }
    let fitted = fitted_bounds(scale, head);
    let origin = Point {
        x: padding + (available_w - (fitted.max_x - fitted.min_x)) / 2.0 - fitted.min_x,
        y: padding + (available_h - (fitted.max_y - fitted.min_y)) / 2.0 - fitted.min_y,
    };
    let mut scaled: Vec<_> = points
        .iter()
        .map(|p| Point {
            x: p.x * scale,
            y: p.y * scale,
        })
        .collect();
    if let Some((size, _)) = head {
        let end = scaled.last_mut().unwrap();
        *end = offset(*end, last, -0.3 * size);
    }
    let mut path = rounded_path(
        &scaled,
        (width.min(height) * 0.1 * non_negative(corner_radius_scale)).min(f64::MAX),
    )?;
    path.transform(1.0, origin);
    let arrowhead = head.filter(|_| shows_arrowhead).map(|(_, arrow)| {
        let place = |p: Point| Point {
            x: p.x + origin.x,
            y: p.y + origin.y,
        };
        Path {
            commands: vec![
                Command::Move(place(arrow[0])),
                Command::Line(place(arrow[1])),
                Command::Line(place(arrow[2])),
                Command::Close,
            ],
        }
    });
    if !path.is_finite() || arrowhead.as_ref().is_some_and(|p| !p.is_finite()) {
        return Err(GeometryError::NonFinite);
    }
    Ok(Geometry { path, arrowhead })
}
