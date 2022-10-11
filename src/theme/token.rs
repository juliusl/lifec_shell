/// Generic tokens that can be used to support colorization directly
/// from a Logos lexer
#[derive(Clone, Debug, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub enum Token {
    Keyword,
    Bracket,
    Operator,
    Modifier,
    Identifier,
    Literal,
    Comment,
    Whitespace,
    Newline,
    Custom(String),
}