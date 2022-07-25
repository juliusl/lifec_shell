use wgpu_glyph::Text;



/// Trait to edit parts of the shell
pub trait ColorTheme {
    fn prompt() -> Text<'static>;
    fn cursor() -> Text<'static>;
    fn background() -> [f32; 4];
    fn red() -> [f32; 4];
    fn blue() -> [f32; 4];
    fn purple() -> [f32; 4];
    fn green() -> [f32; 4];
    fn yellow() -> [f32; 4];
    fn orange() -> [f32; 4];
}