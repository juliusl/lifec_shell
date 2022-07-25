use lifec::Value;
use logos::Lexer;
use logos::Logos;
use lifec::plugins::ThunkContext;
use lifec::AttributeGraphElements;
use lifec::AttributeGraphEvents;
use logos::Span;
use tracing::Level;
use tracing::event;

use crate::DefaultTheme;
use crate::Token;
use crate::theme::ThemeToken;

/// Better runmd language parser, built on top of the v1 parser
/// Builds the attribute graph while lexing
#[derive(Logos, PartialEq, Eq, Debug, Clone)]
#[logos(extras = ThunkContext)]
pub enum Runmd {
    /// Delimits the start or end of a block
    /// 
    /// Cases:
    /// 1) If the delimitter is followed by two tokens, this is the start of a block
    /// 2) If the delimitter is followed by: `md` or `runmd`, the line is ignored and treated as a comment
    /// 3) If nothing follows the delimitter, this denotes the end of a block
    #[token("```", on_block_delimitter)]
    BlockDelimitter(Vec<Span>),
    /// Currently supported block events for this parser:
    /// add, define
    #[token("add", on_block_event)]
    #[token("define", on_block_event)]
    BlockEvent(Vec<Span>),
    /// Attribute values
    #[token(".", on_attribute_value)]
    AttributeValue((Span, Span)),
    /// Coments in runmd
    #[token("``` md", on_comment)]
    #[token("``` runmd", on_comment)]
    #[token("-", on_comment)]
    #[token("#", on_comment)]
    Comment,
    #[regex(r"[ \t\n\f]+", logos::skip)]
    #[error]
    Error,
}

impl Default for Runmd {
    fn default() -> Self {
        Self::Error
    }
}

impl Into<Vec<ThemeToken>> for Runmd {
    fn into(self) -> Vec<ThemeToken> {
        match self {
            Runmd::BlockDelimitter(tokens) => {
                let mut address = vec![];
        
                if let Some(delimitters) = tokens.get(0) {
                    address.push((Token::Bracket, Some(delimitters.clone()))); 
                }

                if tokens.len()  == 3 {
                    if let Some(ident) = tokens.get(1) {
                        address.push((Token::Identifier, Some(ident.clone()))); 
                    }
    
                    if let Some(symbol) = tokens.get(2) {
                        address.push((Token::Keyword, Some(symbol.clone()))); 
                    }
                } else if tokens.len() == 2 {
                    if let Some(symbol) = tokens.get(1) {
                        address.push((Token::Keyword, Some(symbol.clone()))); 
                    }
                }
             
                address
            },
            Runmd::BlockEvent(spans) => {
                let mut tokens = vec![];

                if let Some(event_name) = spans.get(0) {
                    tokens.push((Token::Keyword, Some(event_name.clone()))); 
                }

                if let Some(ident) = spans.get(1) {
                    tokens.push((Token::Identifier, Some(ident.clone()))); 
                }


                if let Some(symbol) = spans.get(2) {
                    tokens.push((Token::Keyword, Some(symbol.clone()))); 
                }

                tokens 
            },
            Runmd::AttributeValue((type_span, literal_span)) => {
                vec![
                    (Token::Keyword, Some(type_span)),
                    (Token::Literal, Some(literal_span))
                ]
            },
            Runmd::Comment => {
                vec![
                    (Token::Comment, None)
                ]
            },
            Runmd::Error => {
                vec![]
            },
        }
    }
}

fn on_comment(lexer: &mut Lexer<Runmd>) -> Option<()> {
    if let Some(eol) = lexer.remainder().find(|c| c == '\r' || c == '\n') {
        let line = &lexer.remainder()[..eol];
        lexer.bump(line.len());
        Some(()) 
    } else {
        None
    }
}

/// Format is typically, bumps the lexer to the value part, but writes to the graph here
/// 
/// {event} {event params ...} {attribute_value}
fn on_block_event(lexer: &mut Lexer<Runmd>) -> Option<Vec<Span>> {
    let event_span = lexer.span();

    let mut tokens = vec![event_span.clone()];

    if let Some(eol) = lexer.remainder().find(|c| c == '\r' || c == '\n') {
        let line = &lexer.remainder()[..eol];

        let elements = AttributeGraphElements::lexer(line);
        let mut event = AttributeGraphEvents::lexer(lexer.slice());

        match event.next() {
            Some(graph_event) => match graph_event {
                // Add event
                // Usage:
                // add {attribute_name} {value_type} {value}
                AttributeGraphEvents::Add => {
                    let mut spanned = elements.spanned();

                    if let (
                            Some((AttributeGraphElements::Symbol(attribute_name), _)),
                            Some((value, value_span))
                      )  = (spanned.next(), spanned.next()) {
                        let value = get_value(value);
                        lexer.extras.as_mut().with(&attribute_name, value.clone());
                        event!(Level::TRACE, "Add event, {attribute_name}, {:?}", value);
                        tokens.push(Span { start: event_span.end, end: event_span.end + value_span.start });
                        lexer.bump(value_span.start);

                    }
                },
    
                // Define event - defines a transient value 
                // Usage:
                // define {attribute_name} {attribute_symbol} (value_type} {value}
                AttributeGraphEvents::Define => {
                    let mut spanned = elements.spanned();

                    if let (
                            Some((AttributeGraphElements::Symbol(attribute_name), name_span)),
                            Some((AttributeGraphElements::Symbol(symbol_name), symbol_span)),
                            Some((value, _))
                      )  = (spanned.next(), spanned.next(), spanned.next()) {
                        event!(Level::TRACE, "Defining event, {attribute_name} {symbol_name}, {:?}", value);
                        let Span { start, end, } = name_span;
                        tokens.push(Span { start: start + event_span.end, end: end + event_span.end + 1 });

                        let Span { start, end, } = symbol_span; 
                        lexer.bump(end);
                        tokens.push(Span { start: start + event_span.end, end: end + event_span.end + 1 });


                        let value = get_value(value);
                        lexer.extras.as_mut()
                            .define(&attribute_name, &symbol_name)
                            .edit_as(value.clone());
                    }
                },
    
                // Currently unsupported events
                AttributeGraphEvents::FindRemove
                | AttributeGraphEvents::Import
                | AttributeGraphEvents::Copy
                | AttributeGraphEvents::Apply
                | AttributeGraphEvents::Edit
                | AttributeGraphEvents::From
                | AttributeGraphEvents::To
                | AttributeGraphEvents::Publish
                | AttributeGraphEvents::Comment
                | AttributeGraphEvents::BlockDelimitter => unreachable!("unsupported events"),
                AttributeGraphEvents::Error => {
                    event!(Level::WARN, "Error parsing, {}", event.slice());
                },
            },
            None => {
                event!(Level::WARN, "Did not parse a supported event, {}", event.slice());
            },
        }

        Some(tokens)
    } else {
        None
    }
}

fn on_attribute_value(lexer: &mut Lexer<Runmd>) -> Option<(Span, Span)> {
    let type_span = lexer.span();
    if let Some(eol) = lexer.remainder().find(|c| c == '\r' || c == '\n') {
        let line = &lexer.remainder()[..eol];
        lexer.bump(line.len());
        let value = &lexer.source()[type_span.start..type_span.end + eol];
        let mut value_type = AttributeGraphElements::lexer(value);
        match value_type.next() {
            Some(element) => {
                match element {
                    AttributeGraphElements::Text(_)
                    | AttributeGraphElements::Bool(_)
                    | AttributeGraphElements::Int(_)
                    | AttributeGraphElements::IntPair(_)
                    | AttributeGraphElements::IntRange(_)
                    | AttributeGraphElements::Float(_)
                    | AttributeGraphElements::FloatPair(_)
                    | AttributeGraphElements::FloatRange(_)
                    | AttributeGraphElements::BinaryVector(_)
                    | AttributeGraphElements::SymbolValue(_) => {
                        let value_type_span = value_type.span();

                        Some((
                            Span { start: type_span.start, end: type_span.end + value_type_span.end }, 
                            Span { start: type_span.end + value_type_span.end, end: type_span.end + eol }))
                    },
                    _ => {
                        None
                    }
                }
            },
            _ => None,
        }
    } else {
        None
    }
}

fn on_block_delimitter(lexer: &mut Lexer<Runmd>) -> Option<Vec<Span>> {
    let delimitter_span = lexer.span();
    if let Some(eol) = lexer.remainder().find(|c| c == '\r' || c == '\n') {
        let line = &lexer.remainder()[..eol];
        lexer.bump(line.len());

        let mut elements = AttributeGraphElements::lexer(line).spanned();
        return match (elements.next(), elements.next()) {
            (
                Some((AttributeGraphElements::Symbol(block_name), name_span)),
                Some((AttributeGraphElements::Symbol(block_symbol), symbol_span)),
            ) => {
                lexer.extras.as_mut().start_block_mode(block_name, block_symbol);
                Some(
                    vec![
                    delimitter_span.clone(),
                    Span { start: delimitter_span.end + name_span.start, end: delimitter_span.end + name_span.end }, 
                    Span { start: delimitter_span.end + symbol_span.start, end: delimitter_span.end + symbol_span.end }
                    ]
                )
            },
            (
                Some((AttributeGraphElements::Symbol(block_symbol), symbol_span)),
                None,
            ) => {

                if let Some(block_name) = lexer.extras.as_ref().find_text("block_name") {
                    lexer.extras.as_mut().start_block_mode(block_name, block_symbol);
                    Some(
                        vec![
                        delimitter_span.clone(),
                        Span { start: delimitter_span.end + symbol_span.start, end: delimitter_span.end + symbol_span.end }
                        ]
                    )
                } else {
                    lexer.extras.as_mut().end_block_mode();
                    Some(
                        vec![
                        delimitter_span,
                        ]
                    ) 
                }
            }
            _ => {
                lexer.extras.as_mut().end_block_mode();
                Some(
                    vec![
                    delimitter_span,
                    ]
                )
            }
        }
    } else {
        lexer.extras.as_mut().end_block_mode();
        Some(
            vec![
            delimitter_span,
            ]
        )
    }
}

fn get_value(element: AttributeGraphElements) -> Value {
    match element {
        AttributeGraphElements::Text(value)
        | AttributeGraphElements::Bool(value)
        | AttributeGraphElements::Int(value)
        | AttributeGraphElements::IntPair(value)
        | AttributeGraphElements::IntRange(value)
        | AttributeGraphElements::Float(value)
        | AttributeGraphElements::FloatPair(value)
        | AttributeGraphElements::FloatRange(value)
        | AttributeGraphElements::BinaryVector(value)
        | AttributeGraphElements::SymbolValue(value) => {
            value
        },
        _ => {
            Value::Empty
        }
    }
}

#[test]
fn test_runmd() {
let runmd = 
r#"
``` demo process
add test_val .text test hello world
define test_val test .text test hello world
``` println
add label .text test label
add duration .int2 5, 6
```
"#;

    // Test lexer
    let mut lexer = Runmd::lexer_with_extras(runmd, ThunkContext::default());
    let token = lexer.next();
    assert_eq!(token, Some(Runmd::BlockDelimitter(vec![(1..5), (5..9), (10..17)])));
    let token = lexer.next();
    assert_eq!(token, Some(Runmd::BlockEvent(vec![(18..21), (21..31)])));
    let _ = lexer.next();
    let token = lexer.next();
    assert_eq!(token, Some(Runmd::BlockEvent(vec![(54..60), (61..70), (70..75)])));
    let token = lexer.next();
    if let Some(Runmd::AttributeValue((_, value_span))) = token.clone() {
        eprintln!("{:?} {}", token, &lexer.source()[value_span]);
    }

    // Test graph creation w/ lexer
    let tc = ThunkContext::default();
    let theme = crate::Theme::new_with::<DefaultTheme>(tc);
     let (tokens, tc) = theme.parse::<Runmd>(runmd);
        for (token, span) in tokens {
            eprintln!("{:?} {}", token, &runmd[span]);
        }

        let project = lifec::plugins::Project::from(tc.as_ref().clone());
        let _ = project.find_block("demo").unwrap(); 
}