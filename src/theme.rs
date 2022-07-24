use logos::Logos;
use wgpu_glyph::Text;

pub struct Theme<'a, L> 
where
    L: Logos<'a>
{
    lexer: &'a mut L
}

impl<'a, L> Theme<'a, L>
where
    L: Logos<'a>
{
    fn apply(buffer: impl AsRef<str>) -> Vec<Text<'a>> {
        vec![]
    }
}