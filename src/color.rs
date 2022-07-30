use wgpu_glyph::Text;

/// Trait to edit parts of the shell
pub trait ColorTheme {
    /// Theme to use for the prompt
    fn prompt() -> Text<'static>;

    /// Theme to use for the cursor
    fn cursor() -> Text<'static>;

    /// Background color,
    /// 
    /// caveat: expecting linear srgb 
    fn background() -> [f32; 4];

    /// Foreground color
    /// 
    /// caveat: expecting linear srgb
    fn foreground() -> [f32; 4];

    /// Red color 
    /// 
    /// caveat: expecting linear srgb
    fn red() -> [f32; 4];

    /// Blue color 
    /// 
    /// caveat: expecting linear srgb
    fn blue() -> [f32; 4];

    /// Purple color
    /// 
    /// caveat: expecting linear srgb
    fn purple() -> [f32; 4];

    /// Green color
    /// 
    /// caveat: expecting linear srgb
    fn green() -> [f32; 4];

    /// Yellow color
    /// 
    /// caveat: expecting linear srgb
    fn yellow() -> [f32; 4];

    /// Orange color
    /// 
    /// caveat: expecting linear srgb
    fn orange() -> [f32; 4];
}