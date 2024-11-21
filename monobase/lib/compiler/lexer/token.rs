use crate::Span;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// Represents a token in the source code, containing both its type and location information.
///
/// Each token consists of:
/// - A span indicating its location in the source code
/// - A kind indicating what type of token it is
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token<'a> {
    /// The location of this token in the source code
    pub span: Span,

    /// The type of this token and any associated data
    pub kind: TokenKind<'a>,
}

/// Represents the different types of tokens that can appear in the source code.
///
/// The lifetime parameter 'a represents borrowed string data from the source code.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TokenKind<'a> {
    /// Regular identifiers: `name`, `value`, etc.
    PlainIdentifier(&'a str),
    /// Identifiers wrapped in backticks: `` `keyword` ``
    EscapedIdentifier(&'a str),
    /// Variables starting with $: `$var`, `$count`
    Variable(&'a str),

    // Literals
    /// Decimal integers: `42`, `1_000`
    DecInteger(&'a str),
    /// Binary integers: `0b1010`, `0b1111_0000`
    BinInteger(&'a str),
    /// Octal integers: `0o755`, `0o777`
    OctInteger(&'a str),
    /// Hexadecimal integers: `0xFF`, `0xDEAD_BEEF`
    HexInteger(&'a str),
    /// Floating point numbers: `3.14`, `1.0e-10`
    Float(&'a str),
    /// String literals: `"hello"`, `'world'`
    String(&'a str),
    /// Byte string literals: `b"bytes"`, `b'data'`
    ByteString(&'a str),
    /// Regular expression literals: `//pattern//flags`
    Regex(&'a str),

    // Delimiters
    /// Opening parenthesis: `(`
    ParenOpen,
    /// Closing parenthesis: `)`
    ParenClose,
    /// Opening square bracket: `[`
    BracketOpen,
    /// Closing square bracket: `]`
    BracketClose,
    /// Opening curly brace: `{`
    BraceOpen,
    /// Closing curly brace: `}`
    BraceClose,
    /// Comma: `,`
    Comma,
    /// Scope operator: `::`
    Scope,
    /// Colon: `:`
    Colon,
    /// Statement terminator: `;`
    Terminator,

    // Assignment Operators
    /// Add and assign: `+=`
    AssignPlus,
    /// Subtract and assign: `-=`
    AssignMinus,
    /// Multiply and assign: `*=` or `×=`
    AssignMul,
    /// Divide and assign: `/=` or `÷=`
    AssignDiv,
    /// Modulo and assign: `%=`
    AssignMod,
    /// Power and assign: `**=`
    AssignPow,
    /// Left shift and assign: `<<=`
    AssignShl,
    /// Right shift and assign: `>>=`
    AssignShr,
    /// Bitwise AND and assign: `&=`
    AssignBitAnd,
    /// Bitwise OR and assign: `|=`
    AssignBitOr,
    /// Bitwise XOR and assign: `^=`
    AssignBitXor,
    /// Bitwise NOT and assign: `~=`
    AssignBitNot,

    // Arrows
    /// Multi-right arrow: `->>`
    MultiArrowRight,
    /// Multi-left arrow: `<<-`
    MultiArrowLeft,
    /// Right arrow: `->`
    ArrowRight,
    /// Left arrow: `<-`
    ArrowLeft,

    // Arithmetic Operators
    /// Addition: `+`
    Plus,
    /// Subtraction: `-`
    Minus,
    /// Multiplication: `×` or `*`
    Mul,
    /// Division: `/` or `÷`
    Div,
    /// Modulo: `%`
    Mod,
    /// Power: `**`
    Pow,

    // Comparison Operators
    /// Pattern match: `~`
    Match,
    /// Pattern not match: `!~`
    NotMatch,
    /// Similarity operator: `<>`
    Similarity,
    /// Logical AND: `&&`
    And,
    /// Logical OR: `||`
    Or,
    /// Equality: `==`
    Eq,
    /// Identity comparison: `=`
    Is,
    /// Negative identity comparison: `!=`
    IsNot,
    /// Logical NOT: `!`
    Not,
    /// Less than or equal: `<=`
    Lte,
    /// Greater than or equal: `>=`
    Gte,
    /// Less than: `<`
    Lt,
    /// Greater than: `>`
    Gt,

    // Set Operators
    /// Contains element: `∋`
    Contains,
    /// Does not contain element: `∌`
    NotContains,
    /// Contains no elements from set: `⊅`
    ContainsNone,
    /// Contains all elements from set: `⊇`
    ContainsAll,
    /// Contains any elements from set: `⊃`
    ContainsAny,

    // Navigation and Special Operators
    /// Safe navigation: `?.`
    SafeNav,
    /// Null coalescing: `?:`
    NullCoalesce,
    /// Left shift: `<<`
    Shl,
    /// Right shift: `>>`
    Shr,
    /// Bitwise AND: `&`
    BitAnd,
    /// Bitwise OR: `|`
    BitOr,
    /// Bitwise XOR: `^`
    BitXor,
    /// Star operator: `*`
    Star,
    /// Dot operator: `.`
    Dot,
    /// Inclusive range: `..=`
    RangeInclusive,
    /// Exclusive range: `..`
    Range,
    /// Optional operator: `?`
    Optional,

    // Special
    /// Module block contents
    ModuleBlock(&'a str),
    /// Comments starting with `--`
    Comment(&'a str),
    /// Whitespace and newlines
    Whitespace(&'a str),
    /// End of file marker
    Eof,

    /// Represents a lexical error with a message
    Error(String),
}
