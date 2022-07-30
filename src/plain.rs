use logos::Logos;
use lifec::plugins::ThunkContext;

use crate::{theme::ThemeToken, Token};

#[derive(Logos, PartialEq, Eq, Debug, Clone)]
#[logos(extras = ThunkContext)]
pub enum Plain {
    Normal,
    #[error]
    Error,
}


impl Into<Vec<ThemeToken>> for Plain {
    fn into(self) -> Vec<ThemeToken> {
        vec![ (Token::Literal, None) ]
    }
}