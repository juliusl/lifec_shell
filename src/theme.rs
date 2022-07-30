use lifec::plugins::ThunkContext;
use logos::{Logos, Span};
use std::{collections::BTreeMap, ops::Range};
use wgpu_glyph::Text;

use crate::{ColorTheme, DefaultTheme, CharDevice};

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

/// Type alias for a theme token
pub type ThemeToken = (Token, Option<Range<usize>>);

#[derive(Default)]
/// Parser that can convert a source into theming tokens
pub struct Theme<Style = DefaultTheme>
where
    Style: ColorTheme + Default,
{
    /// Thunk context
    context: ThunkContext,

    /// Mapping between token and color -- color values should be linear sRGB
    color_map: BTreeMap<Token, [f32; 4]>,

    /// Style
    _style: Style,
}

impl Theme {
    pub fn new() -> Self {
        Theme::new_with(ThunkContext::default())
    }
}

impl<Style> Theme<Style>
where
    Style: ColorTheme + Default,
{
    /// Returns an instance of this theme for a given source, and passes the thunk_context to the lexer
    ///
    /// Parses color symbols to build the color map
    pub fn new_with(tc: ThunkContext) -> Self {
        let mut color_map = BTreeMap::new();
        for (name, value) in tc.as_ref().find_symbol_values("color") {
            let name = name.trim_end_matches("::color");
            color_map.insert(
                match name {
                    "bracket" => Token::Bracket,
                    "operator" => Token::Operator,
                    "modifier" => Token::Modifier,
                    "identifier" => Token::Identifier,
                    "literal" => Token::Literal,
                    "comment" => Token::Comment,
                    "whitespace" => Token::Whitespace,
                    "keyword" => Token::Keyword,
                    custom => Token::Custom(custom.to_string()),
                },
                match value {
                    lifec::Value::FloatRange(r, g, b) => [r, g, b, 1.0],
                    lifec::Value::TextBuffer(color_name) => match color_name.as_str() {
                        "red" => Style::red(),
                        "green" => Style::green(),
                        "blue" => Style::blue(),
                        "purple" => Style::purple(),
                        "yellow" => Style::yellow(),
                        "orange" => Style::orange(),
                        _ => Style::green(),
                    },
                    _ => [1.0, 1.0, 1.0, 1.0],
                },
            );
        }

        color_map.insert(Token::Custom("background".to_string()), Style::background());
        color_map.insert(Token::Custom("red".to_string()), Style::red());
        color_map.insert(Token::Custom("green".to_string()), Style::green());
        color_map.insert(Token::Custom("blue".to_string()), Style::blue());
        color_map.insert(Token::Custom("purple".to_string()), Style::purple());
        color_map.insert(Token::Custom("yellow".to_string()), Style::yellow());
        color_map.insert(Token::Custom("orange".to_string()), Style::orange());

        Self {
            context: tc,
            color_map,
            _style: Style::default(),
        }
    }

    /// Set's the color value (linear sRGB) for the token
    pub fn set_color(&mut self, token: Token, color: [f32; 4]) {
        self.color_map.insert(token, color);
    }

    /// Iterate over current colors for editing
    pub fn colors_mut(&mut self) -> impl Iterator<Item = (&Token, &mut [f32; 4])> {
        self.color_map.iter_mut()
    }

    /// Returns the color for the given token
    pub fn get_color(&self, token: Token) -> Option<&[f32; 4]> {
        self.color_map.get(&token)
    }

    /// Resets the colors to the values set in the current context
    pub fn reset_colors(&mut self) {
        for (name, value) in self.context.as_ref().find_symbol_values("color") {
            let name = name.trim_end_matches("::color");
            self.color_map.insert(
                match name {
                    "bracket" => Token::Bracket,
                    "operator" => Token::Operator,
                    "modifier" => Token::Modifier,
                    "identifier" => Token::Identifier,
                    "literal" => Token::Literal,
                    "comment" => Token::Comment,
                    "whitespace" => Token::Whitespace,
                    "keyword" => Token::Keyword,
                    custom => Token::Custom(custom.to_string()),
                },
                match value {
                    lifec::Value::FloatRange(r, g, b) => [r, g, b, 1.0],
                    lifec::Value::TextBuffer(color_name) => match color_name.as_str() {
                        "red" => Style::red(),
                        "green" => Style::green(),
                        "blue" => Style::blue(),
                        "purple" => Style::purple(),
                        "yellow" => Style::yellow(),
                        "orange" => Style::orange(),
                        _ => Style::green(),
                    },
                    _ => [1.0, 1.0, 1.0, 1.0],
                },
            );
        }
    }

    /// Parses tokens produced by the lexer into tokens used for theming
    ///
    /// If this theme has already been parsed, this is a no op
    pub fn parse<'a, Grammer>(&self, source: &'a str) -> (Vec<(Token, Range<usize>)>, ThunkContext)
    where
        Grammer: Logos<'a, Source = str, Extras = ThunkContext> + Into<Vec<ThemeToken>>,
    {
        let mut lexer = Grammer::lexer_with_extras(source, self.context.clone());
        let mut parsed = vec![];
        let mut cursor = 0;
        while let Some(token) = lexer.next() {
            let tokens: Vec<(Token, Option<Range<usize>>)> = token.into();

            for (token, span) in tokens {
                let span = match span {
                    Some(span) => span,
                    None => lexer.span(),
                };
                cursor = span.end;
                parsed.push((token, span));
            }
        }

        // Appending this to the end ensures that text currently being typed shows up
        parsed.push((
            Token::Whitespace,
            Span {
                start: cursor,
                end: source.len(),
            },
        ));
        (parsed.to_vec(), lexer.extras.clone())
    }

    /// Renders a vector of texts to render/layout
    pub fn render<'a, Grammer>(&self, source: &'a str, prompt_enabled: bool) -> Vec<Text<'a>>
    where
        Grammer: Logos<'a, Source = str, Extras = ThunkContext> + Into<Vec<ThemeToken>>,
    {
        let mut cursor = 0;
        let mut texts = vec![];
        let (tokens, _) = self.parse::<Grammer>(&source);

        if prompt_enabled {
            texts.push(Style::prompt());
        }

        for (token, span) in tokens {
            // Render everything between the cursor and the start of this span
            texts.push(
                Text::new(&source[cursor..span.start])
                    .with_color([1.0, 1.0, 1.0, 0.8])
                    .with_scale(40.0)
                    .with_z(0.8),
            );
            cursor = span.end;

            if span.start < span.end {
                let mut text = Text::new(&source[span]).with_scale(40.0).with_z(0.8);
                if let Some(color) = self.color_map.get(&token) {
                    text = text.with_color(*color);
                } else {
                    text = text.with_color(DefaultTheme::green());
                }
                texts.push(text);
            }
        }

        texts
    }

    pub fn render_cursor<'a>(&self, prompt_enabled: bool) -> impl FnOnce(&'a str, &'a str) -> Vec<Text<'a>> { 
        if prompt_enabled {
           |before, after| {  vec![
                Style::prompt(),
                Text::new(before)
                    .with_color([0.0, 0.0, 0.0, 0.0])
                    .with_scale(40.0)
                    .with_z(0.2),
                Text::new("_")
                    .with_color([0.4, 0.8, 0.8, 1.0])
                    .with_scale(40.0)
                    .with_z(0.2),
                Text::new(after)
                    .with_color([0.0, 0.0, 0.0, 0.0])
                    .with_scale(40.0)
                    .with_z(0.2),
            ]
           }
        } else {
            |before, after| {  vec![
                Text::new(before)
                    .with_color([0.0, 0.0, 0.0, 0.0])
                    .with_scale(40.0)
                    .with_z(0.2),
                Text::new("_")
                    .with_color([0.4, 0.8, 0.8, 1.0])
                    .with_scale(40.0)
                    .with_z(0.2),
                Text::new(after)
                    .with_color([0.0, 0.0, 0.0, 0.0])
                    .with_scale(40.0)
                    .with_z(0.2),
            ]
           }
        }
     
    }
}

mod test {
    use std::ops::Range;

    use crate::Token;
    use lifec::plugins::ThunkContext;
    use logos::Lexer;
    use logos::Logos;
    use logos::Span;

    #[test]
    fn test_theme() {
        let source = r#"
test      abc 
{
// test
. custom
}
"#;
        let mut theme = crate::Theme::new();
        theme.set_color(Token::Bracket, [1.0, 0.0, 0.0, 1.0]);
        theme.set_color(Token::Custom("custom".to_string()), [1.0, 1.0, 0.0, 1.0]);

        let (tokens, _) = theme.parse::<TestGrammer>(source);
        eprintln!("{:#?}", tokens);
        for (token, span) in tokens {
            eprintln!("{:?} {}", token, &source[span]);
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
                    vec![
                        (Token::Modifier, Some(modifier)),
                        (Token::Identifier, Some(ident)),
                    ]
                }
                TestGrammer::TestComment(_) => {
                    vec![(Token::Comment, None)]
                }
                TestGrammer::Error => vec![(Token::Whitespace, None)],
                TestGrammer::TestCustom => vec![(Token::Custom("custom".to_string()), None)],
            }
        }
    }

    fn on_comment(lexer: &mut Lexer<TestGrammer>) -> Option<()> {
        if let Some(eol) = lexer.remainder().find(|c| c == '\r' || c == '\n') {
            let line = &lexer.remainder()[..eol];
            lexer.bump(line.len());
            Some(())
        } else {
            None
        }
    }

    fn on_modifier(lexer: &mut Lexer<TestGrammer>) -> Option<(Span, Span)> {
        if let Some(eol) = lexer
            .remainder()
            .find(|c| c == '\r' || c == '\n' || c == '{')
        {
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
