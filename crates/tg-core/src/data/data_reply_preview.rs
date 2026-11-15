//! Pure geometry fragment from ReplyPreview::prepare. Actual Image/Prepare,
//! spoiler/thumbnail selection/cache and loaded state remain UI5, loader3/4.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreviewGeometry {
    pub width: i32,
    pub height: i32,
    pub outer: i32,
}
pub fn preview_geometry(
    mut width: i32,
    mut height: i32,
    size: i32,
    device_pixel_ratio: i32,
) -> PreviewGeometry {
    if width <= 0 {
        width = 1;
    }
    if height <= 0 {
        height = 1;
    }
    let (w, h) = if width > height {
        (width * size / height, size)
    } else {
        (size, height * size / width)
    };
    PreviewGeometry {
        width: w * device_pixel_ratio,
        height: h * device_pixel_ratio,
        outer: size,
    }
}
