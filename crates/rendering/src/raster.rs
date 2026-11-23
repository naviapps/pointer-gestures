//! Optional CPU rasterization of shared paths. No OS, window or timer is required.
use crate::{Command, Path, Point};
use tiny_skia::{LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform};

/// Invalid raster inputs, excessive dimensions or unrepresentable geometry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RasterError;
impl std::fmt::Display for RasterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("invalid or excessive raster input")
    }
}
impl std::error::Error for RasterError {}

/// Whether a physical-pixel stroke width is positive, at most 4096 and remains
/// positive/finite in the rasterizer's f32 representation. No hairline fallback.
pub fn validate_line_width(value: f64) -> bool {
    value.is_finite() && value > 0.0 && value <= 4096.0 && (value as f32) > 0.0
}

/// An owned tightly packed premultiplied RGBA8 image with transparent background.
pub struct RasterImage {
    pixmap: Pixmap,
}
impl RasterImage {
    /// Width in physical pixels.
    pub fn width(&self) -> u32 {
        self.pixmap.width()
    }
    /// Height in physical pixels.
    pub fn height(&self) -> u32 {
        self.pixmap.height()
    }
    /// Row-major premultiplied RGBA8 bytes, four bytes per pixel.
    pub fn pixels(&self) -> &[u8] {
        self.pixmap.data()
    }
    /// Consume the image and swap red/blue for native premultiplied BGRA surfaces.
    pub fn into_bgra(self) -> Vec<u8> {
        let mut pixels = self.pixmap.take();
        for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel.swap(0, 2);
        }
        pixels
    }
}

/// Draw a round-capped, round-joined antialiased stroke in physical pixels.
///
/// `origin` is the path coordinate at the image's top-left. `scale` converts path
/// units to pixels. Color is straight RGBA8; output is premultiplied. Dimensions
/// must be positive and at most 16384 per axis, with at most 16 million pixels.
/// Width must be finite, positive and at most 4096 pixels. At most 32768 commands
/// are accepted. Malformed sequences or coordinates beyond precise f32 raster
/// range return an error before drawing. Empty paths return a transparent image.
pub fn stroke(
    path: &Path,
    width: u32,
    height: u32,
    origin: Point,
    scale: f64,
    line_width: f64,
    color: [u8; 4],
) -> Result<RasterImage, RasterError> {
    if width == 0
        || height == 0
        || width > 16384
        || height > 16384
        || u64::from(width) * u64::from(height) > 16_000_000
        || !origin.is_finite()
        || !scale.is_finite()
        || scale <= 0.0
        || !validate_line_width(line_width)
        || path.commands.len() > 32768
    {
        return Err(RasterError);
    }
    let convert = |p: Point| -> Result<(f32, f32), RasterError> {
        let x = (p.x - origin.x) * scale;
        let y = (p.y - origin.y) * scale;
        if !x.is_finite() || !y.is_finite() || x.abs() > 16_777_216.0 || y.abs() > 16_777_216.0 {
            return Err(RasterError);
        }
        Ok((x as f32, y as f32))
    };
    let mut builder = PathBuilder::new();
    let mut started = false;
    for command in &path.commands {
        match *command {
            Command::Move(p) => {
                let (x, y) = convert(p)?;
                builder.move_to(x, y);
                started = true;
            }
            Command::Line(p) if started => {
                let (x, y) = convert(p)?;
                builder.line_to(x, y);
            }
            Command::Cubic {
                control1,
                control2,
                to,
            } if started => {
                let (a, b) = convert(control1)?;
                let (c, d) = convert(control2)?;
                let (x, y) = convert(to)?;
                builder.cubic_to(a, b, c, d, x, y);
            }
            Command::Close if started => builder.close(),
            _ => return Err(RasterError),
        }
    }
    let mut pixmap = Pixmap::new(width, height).ok_or(RasterError)?;
    if let Some(path) = builder.finish() {
        let mut paint = Paint::default();
        paint.set_color_rgba8(color[0], color[1], color[2], color[3]);
        let style = Stroke {
            width: line_width as f32,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        pixmap.stroke_path(&path, &paint, &style, Transform::identity(), None);
    }
    Ok(RasterImage { pixmap })
}
