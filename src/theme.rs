use logos::{Lexer, Logos};
use wgpu_glyph::Text;
use std::{collections::HashMap, ops::Range};

use lifec::plugins::ThunkContext;

/// Generic tokens that can be used to support colorization directly
/// from a Logos lexer
#[derive(Clone, Debug, Hash, Eq, PartialEq)]
pub enum Token {
    Keyword,
    Bracket,
    Operator,
    Modifier,
    Identifier,
    Literal,
    Comment,
    Whitespace,
    Custom(String),
}

/// Type alias for a theme token 
pub type ThemeToken = (Token, Option<Range<usize>>);

/// Parser that can convert a source into theming tokens
pub struct Theme<'a, Grammer>
where
    Grammer: Logos<'a, Source = str, Extras = ThunkContext> + Into<Vec<ThemeToken>>,
{
    /// Lexer for finding token positions
    lexer: Option<Lexer<'a, Grammer>>,

    /// Thunk context
    context: Option<ThunkContext>,

    /// Source used to create the lexer
    source: &'a str, 

    /// Tokens parsed from lexer, 
    /// 
    /// If lexer is Some, then this could be empty
    tokens: Vec<(Token, Range<usize>)>,

    /// Mapping between token and color -- color values should be linear sRGB
    color_map: HashMap<Token, [f32; 4]>,
}

impl<'a, Grammer> Theme<'a, Grammer>
where
    Grammer: Logos<'a, Source = str, Extras = ThunkContext> + Into<Vec<ThemeToken>>,
{
    /// Returns an instance of this theme for a given source
    pub fn new(source: &'a str) -> Self {
        Self {
            lexer: Some(Grammer::lexer(source)),
            context: None,
            source,
            tokens: vec![],
            color_map: Default::default(),
        }
    }

    /// Returns an instance of this theme for a given source, and passes the thunk_context to the lexer
    /// 
    /// Parses color symbols to build the color map
    pub fn new_with(source: &'a str, tc: ThunkContext) -> Self {
       let mut color_map = HashMap::new();
       for (name, value) in tc.as_ref().find_symbol_values("color") {
            let name = name.trim_end_matches("::color");
            match value {
                lifec::Value::FloatRange(r, g, b) => {
                    let color = [r, g, b, 1.0];
                    color_map.insert(match name {
                        "bracket" => Token::Bracket,
                        "operator" => Token::Operator,
                        "modifier" => Token::Modifier,
                        "identifier" => Token::Identifier, 
                        "literal" => Token::Literal,
                        "comment" => Token::Comment,
                        "whitespace" => Token::Whitespace,
                        "keyword" => Token::Keyword,
                        custom => Token::Custom(custom.to_string()),
                    }, color);
                },
                _ => {}
            }
       }
       let lexer = Some(Grammer::lexer_with_extras(source, tc));

        Self {
            lexer,
            context: None,
            source,
            tokens: vec![],
            color_map,
        }
    }

    /// Set's the color value (linear sRGB) for the token
    pub fn set_color(&mut self, token: Token, color: [f32; 4]) {
        self.color_map.insert(token, color);
    }

    /// Parses tokens produced by the lexer into tokens used for theming
    /// 
    /// If this theme has already been parsed, this is a no op
    pub fn parse(&mut self) -> Option<(Vec<(Token, Range<usize>)>, ThunkContext)> {
        let mut parsed = vec![];
        if let Some(mut lexer) = self.lexer.take() {
            while let Some(token) = lexer.next() {
                let tokens: Vec<(Token, Option<Range<usize>>)> = token.into();

                for (token, span) in tokens {
                    let span = match span {
                        Some(span) => span,
                        None => lexer.span(),
                    };
                    parsed.push((token, span));
                }
            }

            self.tokens = parsed.to_vec();
            self.context = Some(lexer.extras);

        }

        if let Some(context) = self.context.as_ref() {
            Some((self.tokens.to_vec(), context.clone()))
        } else {
            None 
        }
    }

    /// Renders a vector of texts to render/layout
    pub fn render(&mut self) -> Vec<Text<'a>> {
        let mut texts = vec![];

        if let Some((tokens, _)) = self.parse() {
            for (token, span) in tokens {
                let mut text = Text::new(&self.source[span]);

                if let Some(color) = self.color_map.get(&token) {
                    text = text.with_color(*color);
                }
    
                texts.push(text);
            }
        }
        texts
    }
}

mod test {
    use std::ops::Range;

    use logos::Lexer;
    use logos::Logos;
    use logos::Span;
    use crate::Token;
    use lifec::plugins::ThunkContext;

    #[test]
    fn test_theme() {
        let source = r#"
test      abc 
{
// test
. custom
}
"#;
        let mut theme = crate::Theme::<TestGrammer>::new(source);
        theme.set_color(Token::Bracket, [1.0, 0.0, 0.0, 1.0]);
        theme.set_color(Token::Custom("custom".to_string()), [1.0, 1.0, 0.0, 1.0]);

        if let Some((tokens, _)) = theme.parse() {
            eprintln!("{:#?}", tokens);
            for (token, span) in tokens {
                eprintln!("{:?} {}", token, &source[span]);
            }
        }

    }

    #[derive(Logos, PartialEq, Eq)]
    #[logos(extras = ThunkContext)]
    enum TestGrammer {
        #[token("{")]
        #[token("}")]
        TestBracket,
        #[token(".")]
        TestOperator,
        #[token("test", on_modifier)]
        TestModifier((Span, Span)),
        #[token("//", on_comment)]
        TestComment(()),
        #[token("custom")]
        TestCustom,
        #[error]
        #[regex(r"[ \t\n\f]+", logos::skip)]
        Error,
    }

    impl Into<Vec<(Token, Option<Range<usize>>)>> for TestGrammer {
        fn into(self) -> Vec<(Token, Option<Range<usize>>)> {
            match self {
                TestGrammer::TestBracket => vec![(Token::Bracket, None)],
                TestGrammer::TestOperator => vec![(Token::Operator, None)],
                TestGrammer::TestModifier((modifier, ident)) => {
                    vec![(Token::Modifier, Some(modifier)), (Token::Identifier, Some(ident))]
                },
                TestGrammer::TestComment(_) => {
                    vec![(Token::Comment, None)]
                }
                TestGrammer::Error => vec![(Token::Whitespace, None)],
                TestGrammer::TestCustom => vec![(Token::Custom("custom".to_string()), None)]
            }
        }
    }

    fn on_comment(lexer: &mut Lexer<TestGrammer>) -> Option<()>{
        if let Some(eol) = lexer.remainder().find(|c| c == '\r' || c == '\n') {
            let line = &lexer.remainder()[..eol];
            lexer.bump(line.len());
            Some(()) 
        } else {
            None
        }
    }

    fn on_modifier(lexer: &mut Lexer<TestGrammer>) -> Option<(Span, Span)> {
        if let Some(eol) = lexer.remainder().find(|c| c == '\r' || c == '\n' || c == '{' ) {
            let line = &lexer.remainder()[..eol];
            let modifier = lexer.span();
            lexer.bump(line.len());

            let Range { start, end } = lexer.span();

            let keyword_start = modifier.end - start + 1;
            Some((modifier, (keyword_start..end)))
        } else {
            None
        }
    }
}
