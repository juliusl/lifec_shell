use wgpu_glyph::Text;

use crate::ColorTheme;


#[derive(Default, Clone)]
pub struct DefaultTheme;

impl ColorTheme for DefaultTheme {
    fn prompt() -> Text<'static> {
        Text::new("> ")
            .with_color([1.0, 0.0, 0.0, 1.0])
            .with_scale(40.0)
    }

    fn cursor() -> Text<'static> {
        Text::new("_")
            .with_color([0.4, 0.8, 0.8, 1.0])
            .with_scale(40.0)
            .with_z(0.2)
    }

    fn background() -> [f32; 4] {
        [0.02122, 0.02519, 0.03434, 1.0]
    }

    fn foreground() -> [f32; 4] {
        Self::yellow()
    }

    fn red() -> [f32; 4] {
        [0.7454, 0.14996, 0.17789, 1.0]
    }

    fn blue() -> [f32; 4] {
        [0.11954, 0.42869, 0.86316, 1.0]
    }

    fn purple() -> [f32; 4] {
        [0.56471, 0.18782, 0.72306, 1.0]
    }

    fn green() -> [f32; 4] {
        [0.31399, 0.54572, 0.1912, 1.0]
    }

    fn yellow() -> [f32; 4] {
        [0.78354, 0.52712, 0.19807, 1.0]
    }

    fn orange() -> [f32; 4] {
        [0.78354, 0.52712, 0.19807, 1.0]
    }
}
