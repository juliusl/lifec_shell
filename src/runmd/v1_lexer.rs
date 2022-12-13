use lifec::prelude::Value;
use logos::Logos;

#[derive(Logos, Debug, Hash, Clone, PartialEq, PartialOrd)]
pub enum AttributeGraphEvents {
    /// Usage: add {`attribute-name`} {`value-type`} {`remaining as value`}
    /// Example: add test_attr .TEXT remaining text is text
    /// Adds a new attribute to the graph. Types omitted from this event are symbol, reference, and binary-vector
    #[token("add")]
    #[token("+")]
    Add,
    /// Usage: define {`attribute-name`} {`symbol-name`}
    /// Examples: define test_attr node
    /// Defines and adds a symbol for a specified attribute name
    /// If the attribute doesn't already exist, it is not added.
    /// The format of the name for the symbol attribute is {`attribute-name`}::{`symbol-name`}
    /// The value of the symbol will be {`symbol-name`}::
    #[token("define")]
    #[token(":")]
    Define,
    /// Usage:   # Here is a helpful comment
    ///          - Here is another helpful comment
    ///         // Here is anothet helpful comment
    ///     ``` md Here is another helpful comment
    ///  ``` runmd Here is another helpful comment
    #[token("#")]
    #[token("-")]
    #[token("//")]
    #[token("``` md")]
    #[token("``` runmd")]
    Comment,
    /// Usage:
    /// add test_attr .TEXT remaining text is the value
    ///
    #[token("```")]
    BlockDelimitter,
    // Logos requires one token variant to handle errors,
    // it can be named anything you wish.
    #[error]
    // We can also use this variant to define whitespace,
    // or any other matches we wish to skip.
    #[regex(r"[ \t\n\f]+", logos::skip)]
    Error,
}

/// Elements contained within an attribute graph
#[derive(Logos, Debug, Hash, Clone, PartialEq, PartialOrd)]
pub enum AttributeGraphElements {
    /// text element parses all remaining text after .TEXT as a string
    #[token(".text", graph_lexer::from_text)]
    #[token(".TEXT", graph_lexer::from_text)]
    Text(Value),
    /// bool element parses remaining as bool
    #[token(".enable", |_| Value::Bool(true))]
    #[token(".disable", |_| Value::Bool(false))]
    #[token(".bool", graph_lexer::from_bool)]
    #[token(".BOOL", graph_lexer::from_bool)]
    Bool(Value),
    /// int element parses remaining as i32
    #[token(".int", graph_lexer::from_int)]
    #[token(".INT", graph_lexer::from_int)]
    Int(Value),
    /// int pair element parses remaining as 2 comma-delimmited i32's
    #[token(".int2", graph_lexer::from_int_pair)]
    #[token(".INT_PAIR", graph_lexer::from_int_pair)]
    IntPair(Value),
    /// int range element parses remaining as 3 comma-delimitted i32's
    #[token(".int3", graph_lexer::from_int_range)]
    #[token(".int_range", graph_lexer::from_int_range)]
    #[token(".INT_RANGE", graph_lexer::from_int_range)]
    IntRange(Value),
    /// float element parses remaining as f32
    #[token(".float", graph_lexer::from_float)]
    #[token(".FLOAT", graph_lexer::from_float)]
    Float(Value),
    /// float pair element parses reamining as 2 comma delimitted f32's
    #[token(".float2", graph_lexer::from_float_pair)]
    #[token(".FLOAT_PAIR", graph_lexer::from_float_pair)]
    FloatPair(Value),
    /// float range element parses remaining as 3 comma delimitted f32's
    #[token(".float3", graph_lexer::from_float_range)]
    #[token(".FLOAT_RANGE", graph_lexer::from_float_range)]
    FloatRange(Value),
    /// binary vector element, currently parses the remaining as base64 encoded data
    #[token(".bin", graph_lexer::from_binary_vector_base64)]
    #[token(".base64", graph_lexer::from_binary_vector_base64)]
    #[token(".BINARY_VECTOR", graph_lexer::from_binary_vector_base64)]
    BinaryVector(Value),
    /// symbol value implies that the value is of symbolic quality, 
    /// and though no explicit validations are in place, the value of the symbol
    /// should be valid in many contexts that require an identifier
    #[token(".symbol", graph_lexer::from_symbol)]
    SymbolValue(Value),
    /// empty element parses
    #[token(".empty")]
    #[token(".EMPTY")]
    Empty,
    /// entity ids should be parsed before symbols
    #[regex("[0-9]+", priority = 3, callback = graph_lexer::from_entity)]
    Entity(u32),
    /// symbols must start with a character, and is composed of lowercase characters, digits, underscores, and colons
    #[regex("[A-Za-z]+[A-Za-z-._:0-9]*", graph_lexer::from_string)]
    Symbol(String),
    /// names have more relaxed rules
    #[regex("[#][A-Za-z_.-/0-9]*", graph_lexer::from_string)]
    Name(String),
    // Logos requires one token variant to handle errors,
    // it can be named anything you wish.
    #[error]
    // We can also use this variant to define whitespace,
    // or any other matches we wish to skip.
    #[regex(r"[ \t\n\f]+", logos::skip)]
    Error,
}

mod graph_lexer {
    use std::str::FromStr;

    use lifec::prelude::Value;
    use logos::Lexer;

    use super::AttributeGraphElements;

    pub fn from_entity(lexer: &mut Lexer<AttributeGraphElements>) -> Option<u32> {
        lexer.slice().parse().ok()
    }

    pub fn from_symbol(lexer: &mut Lexer<AttributeGraphElements>) -> Option<Value> {
        let remaining = lexer.remainder().trim().to_string();

        Some(Value::Symbol(remaining))
    }

    pub fn from_string(lexer: &mut Lexer<AttributeGraphElements>) -> Option<String> {
        let mut slice = lexer.slice();
        if slice.starts_with('#') {
            slice = &slice[1..];
        }

        Some(slice.to_string())
    }
    pub fn from_text(lexer: &mut Lexer<AttributeGraphElements>) -> Option<Value> {
        let remaining = lexer.remainder().trim().to_string();

        Some(Value::TextBuffer(remaining))
    }

    pub fn from_bool(lexer: &mut Lexer<AttributeGraphElements>) -> Option<Value> {
        if let Some(value) = lexer.remainder().trim().parse().ok() {
            Some(Value::Bool(value))
        } else {
            None
        }
    }

    pub fn from_int(lexer: &mut Lexer<AttributeGraphElements>) -> Option<Value> {
        if let Some(value) = lexer.remainder().trim().parse::<i32>().ok() {
            Some(Value::Int(value))
        } else {
            None
        }
    }

    pub fn from_int_pair(lexer: &mut Lexer<AttributeGraphElements>) -> Option<Value> {
        let pair = from_comma_sep::<i32>(lexer);

        match (pair.get(0), pair.get(1)) {
            (Some(f0), Some(f1)) => Some(Value::IntPair(*f0, *f1)),
            _ => None,
        }
    }

    pub fn from_int_range(lexer: &mut Lexer<AttributeGraphElements>) -> Option<Value> {
        let range = from_comma_sep::<i32>(lexer);

        match (range.get(0), range.get(1), range.get(2)) {
            (Some(f0), Some(f1), Some(f2)) => Some(Value::IntRange(*f0, *f1, *f2)),
            _ => None,
        }
    }

    pub fn from_float(lexer: &mut Lexer<AttributeGraphElements>) -> Option<Value> {
        if let Some(value) = lexer.remainder().trim().parse::<f32>().ok() {
            Some(Value::Float(value))
        } else {
            None
        }
    }

    pub fn from_float_pair(lexer: &mut Lexer<AttributeGraphElements>) -> Option<Value> {
        let pair = from_comma_sep::<f32>(lexer);
        match (pair.get(0), pair.get(1)) {
            (Some(f0), Some(f1)) => Some(Value::FloatPair(*f0, *f1)),
            _ => None,
        }
    }

    pub fn from_float_range(lexer: &mut Lexer<AttributeGraphElements>) -> Option<Value> {
        let range = from_comma_sep::<f32>(lexer);

        match (range.get(0), range.get(1), range.get(2)) {
            (Some(f0), Some(f1), Some(f2)) => Some(Value::FloatRange(*f0, *f1, *f2)),
            _ => None,
        }
    }

    pub fn from_binary_vector_base64(lexer: &mut Lexer<AttributeGraphElements>) -> Option<Value> {
        match base64::decode(lexer.remainder().trim()) {
            Ok(content) => Some(Value::BinaryVector(content)),
            Err(_) => None,
        }
    }

    fn from_comma_sep<T>(lexer: &mut Lexer<AttributeGraphElements>) -> Vec<T>
    where
        T: FromStr,
    {
        lexer
            .remainder()
            .trim()
            .split(",")
            .filter_map(|i| i.trim().parse().ok())
            .collect()
    }
}