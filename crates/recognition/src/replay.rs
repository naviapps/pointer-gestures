//! Bounded replay sampling shared by native input adapters.
use crate::Point;

/// Number of sampled replay points. Preserve both endpoints even for limits below two.
pub fn replay_point_count(count: usize, maximum: usize) -> usize {
    count.min(maximum.max(2))
}

/// Sample a retained path at evenly spaced indices, preserving origin and final
/// point. Integer arithmetic keeps indices exact even for very large input counts.
pub fn replay_points(points: &[Point], maximum: usize) -> Vec<Point> {
    let count = replay_point_count(points.len(), maximum);
    if count == points.len() {
        return points.to_vec();
    }
    (0..count)
        .map(|index| {
            let source =
                (index as u128 * (points.len() - 1) as u128 / (count - 1) as u128) as usize;
            points[source]
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_sampling_keeps_endpoints_and_exact_spacing() {
        let path: Vec<_> = (0..100)
            .map(|x| Point {
                x: f64::from(x),
                y: 0.,
            })
            .collect();
        let sample = replay_points(&path, 4);
        assert_eq!(
            sample.iter().map(|p| p.x).collect::<Vec<_>>(),
            [0., 33., 66., 99.]
        );
        assert_eq!(replay_points(&path, 0), [path[0], path[99]]);
        assert_eq!(replay_points(&path[..1], 0), path[..1]);
        assert!(replay_points(&[], 64).is_empty());
        assert_eq!(replay_point_count(usize::MAX, usize::MAX), usize::MAX);
    }
}
