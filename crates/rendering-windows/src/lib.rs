//! Optional transparent Windows desktop overlay. Shared geometry, rasterization
//! and presentation cadence stay in Rust; hosts own activation and appearance.
#![warn(missing_docs)]
#[cfg(windows)]
mod desktop;
#[cfg(windows)]
mod window;
#[cfg(windows)]
pub use desktop::{Overlay, WindowState};
use pointer_gesture_rendering::{CornerStyle, OverlayTiming};

/// Host-supplied appearance in physical pixels, independent of product policy.
#[derive(Debug, Clone, Copy)]
pub struct Configuration {
    /// Straight red, green, blue and alpha bytes.
    pub color: [u8; 4],
    /// Positive f32-representable stroke width in physical pixels, at most 4096.
    pub line_width: f64,
    /// Shared live-trace corner geometry.
    pub corners: CornerStyle,
    /// Shared movement coalescing and final-trace cadence.
    pub timing: OverlayTiming,
}
impl Configuration {
    /// Validate appearance before allocating any OS resources.
    pub fn validate(self) -> Result<Self, Error> {
        if !pointer_gesture_rendering::raster::validate_line_width(self.line_width)
            || !self.corners.radius_scale.is_finite()
            || self.corners.radius_scale < 0.
        {
            return Err(Error::InvalidConfiguration);
        }
        Ok(Self {
            timing: OverlayTiming::new(self.timing.interval_seconds, self.timing.maximum_point_lag),
            ..self
        })
    }
}

/// Native overlay failures. A failed presentation hides stale geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Invalid appearance, excessive trace or raster bounds.
    InvalidConfiguration,
    /// Activate before submitting geometry or querying native state.
    Inactive,
    /// A native operation failed with its Windows error code.
    System(u32),
    /// The owner thread ended before a request completed.
    WorkerStopped,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Windows gesture overlay: {self:?}")
    }
}
impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn appearance_is_validated_before_native_resource_creation() {
        let configuration = Configuration {
            color: [0, 20, 255, 128],
            line_width: 4.,
            corners: CornerStyle::default(),
            timing: OverlayTiming::default(),
        };
        assert_eq!(configuration.validate().unwrap().line_width, 4.);
        for width in [f64::MIN_POSITIVE, 0., -1., f64::NAN, f64::INFINITY, 4097.] {
            assert!(matches!(
                Configuration {
                    line_width: width,
                    ..configuration
                }
                .validate(),
                Err(Error::InvalidConfiguration)
            ));
        }
        for radius_scale in [-1., f64::NAN, f64::INFINITY] {
            assert!(matches!(
                Configuration {
                    corners: CornerStyle { radius_scale },
                    ..configuration
                }
                .validate(),
                Err(Error::InvalidConfiguration)
            ));
        }
        let normalized = Configuration {
            timing: OverlayTiming {
                interval_seconds: f64::NAN,

                maximum_point_lag: -1.,
            },
            ..configuration
        }
        .validate()
        .unwrap();
        assert_eq!(
            normalized.timing.interval_seconds,
            OverlayTiming::default().interval_seconds
        );
        assert_eq!(normalized.timing.maximum_point_lag, 24.);
    }
}
