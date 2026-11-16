//! Color data only. QColor color-space conversion/painting remains stage5.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Color {
    pub rgba: u32,
    pub valid: bool,
}
impl Default for Color {
    fn default() -> Self {
        Self {
            rgba: 255,
            valid: false,
        }
    }
}
impl Color {
    pub const fn rgb(rgb: u32) -> Self {
        Self {
            rgba: ((rgb & 0xffffff) << 8) | 255,
            valid: true,
        }
    }
    pub const fn from_rgba(rgba: u32) -> Self {
        Self { rgba, valid: true }
    }
    pub const fn serialized(self) -> u32 {
        self.rgba >> 8
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ColorProfileSet {
    pub palette: Vec<Color>,
    pub bg: Vec<Color>,
    pub story: Vec<Color>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ColorProfileData {
    pub light: ColorProfileSet,
    pub dark: ColorProfileSet,
}
