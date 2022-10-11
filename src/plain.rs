use logos::Logos;

use crate::{theme::ThemeToken, Token};

#[derive(Logos, PartialEq, Eq, Debug, Clone)]
#[logos(extras = ())]
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