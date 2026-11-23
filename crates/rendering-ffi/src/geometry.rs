use crate::input::{buffer, directions};
use pointer_gesture_rendering::{
    Bounds, Command, CornerStyle, Geometry, GeometryError, Path, Point,
};

#[repr(C)]
pub struct NativeCommand {
    kind: u32,
    point: Point,
    control1: Point,
    control2: Point,
}
pub struct GeometryHandle {
    path: Vec<NativeCommand>,
    arrowhead: Vec<NativeCommand>,
}
#[repr(C)]
pub struct GeometryResult {
    pub code: u32,
    pub geometry: *mut GeometryHandle,
}
#[repr(C)]
pub struct GeometryView {
    pub path: *const NativeCommand,
    pub path_count: usize,
    pub arrowhead: *const NativeCommand,
    pub arrowhead_count: usize,
}
#[repr(C)]
pub struct BoundsResult {
    pub valid: u32,
    pub bounds: Bounds,
}

const ZERO: Point = Point { x: 0.0, y: 0.0 };

fn native_commands(path: Path) -> Vec<NativeCommand> {
    path.commands
        .into_iter()
        .map(|command| match command {
            Command::Move(point) => NativeCommand {
                kind: 1,
                point,
                control1: ZERO,
                control2: ZERO,
            },
            Command::Line(point) => NativeCommand {
                kind: 2,
                point,
                control1: ZERO,
                control2: ZERO,
            },
            Command::Cubic {
                control1,
                control2,
                to,
            } => NativeCommand {
                kind: 3,
                point: to,
                control1,
                control2,
            },
            Command::Close => NativeCommand {
                kind: 4,
                point: ZERO,
                control1: ZERO,
                control2: ZERO,
            },
        })
        .collect()
}
impl GeometryResult {
    fn from_geometry(value: Result<Geometry, GeometryError>) -> Self {
        match value {
            Ok(value) => Self {
                code: 0,
                geometry: Box::into_raw(Box::new(GeometryHandle {
                    path: native_commands(value.path),
                    arrowhead: value.arrowhead.map(native_commands).unwrap_or_default(),
                })),
            },
            Err(error) => Self {
                code: error as u32,
                geometry: std::ptr::null_mut(),
            },
        }
    }
}

/// # Safety
/// Non-null input buffers must be readable for their counts for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_live_trace(
    data: *const u32,
    count: usize,
    points: *const Point,
    point_count: usize,
    radius_scale: f64,
    tail: Point,
    tail_present: u32,
) -> GeometryResult {
    GeometryResult::from_geometry((|| {
        let directions = unsafe { directions(data, count) }?;
        let points = unsafe { buffer(points, point_count) }?;
        if tail_present > 1 {
            return Err(GeometryError::InvalidBuffer);
        }
        let path = pointer_gesture_rendering::live_trace(
            directions,
            points,
            (tail_present != 0).then_some(tail),
            CornerStyle { radius_scale },
        )?;
        Ok(Geometry {
            path,
            arrowhead: None,
        })
    })())
}

/// # Safety
/// The direction buffer must be readable for `count` elements for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_pattern(
    data: *const u32,
    count: usize,
    width: f64,
    height: f64,
    padding: f64,
    line_width: f64,
    arrowhead: u32,
    corner_radius_scale: f64,
) -> GeometryResult {
    GeometryResult::from_geometry((|| {
        if arrowhead > 1 {
            return Err(GeometryError::InvalidBuffer);
        }
        pointer_gesture_rendering::pattern(
            unsafe { directions(data, count) }?,
            width,
            height,
            padding,
            line_width,
            arrowhead != 0,
            corner_radius_scale,
        )
    })())
}

/// # Safety
/// `geometry` is null or a live immutable handle. Its command views remain valid
/// until destruction, which must not overlap any reader.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_geometry_view(
    geometry: *const GeometryHandle,
) -> GeometryView {
    if geometry.is_null() {
        return GeometryView {
            path: std::ptr::null(),
            path_count: 0,
            arrowhead: std::ptr::null(),
            arrowhead_count: 0,
        };
    }
    let geometry = unsafe { &*geometry };
    GeometryView {
        path: if geometry.path.is_empty() {
            std::ptr::null()
        } else {
            geometry.path.as_ptr()
        },
        path_count: geometry.path.len(),
        arrowhead: if geometry.arrowhead.is_empty() {
            std::ptr::null()
        } else {
            geometry.arrowhead.as_ptr()
        },
        arrowhead_count: geometry.arrowhead.len(),
    }
}

/// # Safety
/// `geometry` is null or an exclusively owned live handle, destroyed exactly once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_geometry_destroy(geometry: *mut GeometryHandle) {
    if !geometry.is_null() {
        drop(unsafe { Box::from_raw(geometry) });
    }
}

/// # Safety
/// The point buffer must be readable for `count` elements for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pointer_gesture_rendering_bounds(
    points: *const Point,
    count: usize,
) -> BoundsResult {
    let bounds = unsafe { buffer(points, count) }
        .ok()
        .and_then(Bounds::from_points);
    BoundsResult {
        valid: u32::from(bounds.is_some()),
        bounds: bounds.unwrap_or(Bounds {
            min_x: 0.0,
            min_y: 0.0,
            max_x: 0.0,
            max_y: 0.0,
        }),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pointer_gesture_rendering_intersects(
    bounds: Bounds,
    tail: Point,
    tail_present: u32,
    viewport: Bounds,
    line_width: f64,
) -> u32 {
    u32::from(
        tail_present <= 1
            && bounds.intersects((tail_present != 0).then_some(tail), viewport, line_width),
    )
}
