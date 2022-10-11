use super::ThemeToken;

/// Trait to apply syntax coloring,
/// 
pub trait Grammer {
    /// Parse string content into tokens,
    /// 
    fn parse(&self, content: impl AsRef<str>) -> Vec<ThemeToken>;
}