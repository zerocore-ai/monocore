use super::token::{Token, TokenKind};
use crate::Span;

//--------------------------------------------------------------------------------------------------
// Types
//--------------------------------------------------------------------------------------------------

/// A lexical analyzer (lexer) that converts source code into a sequence of tokens.
///
/// The lexer processes source text character by character to produce tokens according to the language
/// grammar. It handles:
/// - Keywords and identifiers
/// - Numbers (decimal, hex, octal, binary, floating point)
/// - Strings with escape sequences
/// - Operators and delimiters
/// - Comments and whitespace
///
/// ## Fields
///
/// * `source` - The input source code being lexed
/// * `pos` - Current position in the source text (in bytes)
/// * `start` - Starting position of the current token being lexed (in bytes)
/// * `line` - Current line number (1-based)
/// * `column` - Current column number (1-based)
///
/// ## Examples
///
/// ```
/// use monobase::compiler::Lexer;
///
/// let mut lexer = Lexer::new("let x = 42");
/// let token = lexer.next_token();
/// ```
pub struct Lexer<'a> {
    /// Source code being lexed
    source: &'a str,

    /// Current position in source (in bytes)
    pos: usize,

    /// Start of current token (in bytes)
    start: usize,

    /// Current line number (1-based)
    line: usize,

    /// Current column number (1-based)
    column: usize,
}

//--------------------------------------------------------------------------------------------------
// Methods
//--------------------------------------------------------------------------------------------------

impl<'a> Lexer<'a> {
    /// Creates a new lexer for the given source code.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monobase::compiler::Lexer;
    ///
    /// let lexer = Lexer::new("let x = 42");
    /// ```
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            pos: 0,
            start: 0,
            line: 1,
            column: 1,
        }
    }

    /// Returns the next token from the source code.
    ///
    /// This method advances through the source code, skipping whitespace and comments,
    /// and returns the next meaningful token. Returns `TokenKind::Eof` when reaching
    /// the end of the source.
    ///
    /// ## Examples
    ///
    /// ```
    /// use monobase::compiler::{Lexer, TokenKind};
    ///
    /// let mut lexer = Lexer::new("$count += 1");
    ///
    /// // Get variable token
    /// let token = lexer.next_token();
    /// assert!(matches!(token.kind, TokenKind::Variable("$count")));
    ///
    /// // Get operator token
    /// let token = lexer.next_token();
    /// assert!(matches!(token.kind, TokenKind::AssignPlus));
    ///
    /// // Get number token
    /// let token = lexer.next_token();
    /// assert!(matches!(token.kind, TokenKind::DecInteger("1")));
    /// ```
    pub fn next_token(&mut self) -> Token<'a> {
        self.skip_whitespace();

        self.start = self.pos;

        let c = match self.peek() {
            Some(c) => c,
            None => return self.make_token(TokenKind::Eof),
        };

        match c {
            // Single character tokens
            '(' => {
                self.advance();
                self.make_token(TokenKind::ParenOpen)
            }
            ')' => {
                self.advance();
                self.make_token(TokenKind::ParenClose)
            }
            '[' => {
                self.advance();
                self.make_token(TokenKind::BracketOpen)
            }
            ']' => {
                self.advance();
                self.make_token(TokenKind::BracketClose)
            }
            '{' => {
                self.advance();
                self.make_token(TokenKind::BraceOpen)
            }
            '}' => {
                self.advance();
                self.make_token(TokenKind::BraceClose)
            }
            ',' => {
                self.advance();
                self.make_token(TokenKind::Comma)
            }
            ';' => {
                self.advance();
                self.make_token(TokenKind::Terminator)
            }

            // Byte strings (must come before identifier check)
            'b' if self.peek_ahead(1) == Some('"') || self.peek_ahead(1) == Some('\'') => {
                self.byte_string()
            }

            // Identifiers and keywords (moved after byte string check)
            'a'..='z' | 'A'..='Z' | '_' => self.identifier(),

            // Numbers
            '0'..='9' => self.number(),

            // Strings
            '"' | '\'' => self.string(),

            // Escaped identifiers
            '`' => self.escaped_identifier(),

            // Regex literals
            '/' if self.peek_ahead(1) == Some('/') => self.regex(),

            // Operators
            '+' => self.operator_plus(),
            '-' => self.operator_minus(),
            '*' => self.operator_star(),
            '/' => self.operator_slash(),
            '=' => self.operator_equals(),
            '<' => self.operator_less(),
            '>' => self.operator_greater(),
            '&' => self.operator_amp(),
            '|' => self.operator_pipe(),
            '^' => self.operator_caret(),
            '~' => self.operator_tilde(),
            '!' => self.operator_bang(),
            '?' => self.operator_question(),
            ':' => self.operator_colon(),
            '.' => self.operator_dot(),
            '$' => self.variable(),

            // Add modulo operator
            '%' => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    self.make_token(TokenKind::AssignMod)
                } else {
                    self.make_token(TokenKind::Mod)
                }
            }

            // Add Unicode operators
            '×' => self.operator_mul_lexer(),
            '÷' => self.operator_div_lexer(),
            '∋' => self.operator_contains_lexer(),
            '∌' => self.operator_not_contains_lexer(),
            '⊅' => self.operator_contains_none_lexer(),
            '⊇' => self.operator_contains_all_lexer(),
            '⊃' => self.operator_contains_any_lexer(),

            // Invalid character
            c => {
                self.advance();
                self.make_token(TokenKind::Error(format!("Unexpected character: '{}'", c)))
            }
        }
    }

    /// Handles numeric literals including integers and floating point numbers.
    ///
    /// Supports:
    /// - Decimal integers: `123`, `1_000`
    /// - Hex integers: `0xFF`, `0xDEAD_BEEF`
    /// - Binary integers: `0b1010`, `0b1111_0000`
    /// - Octal integers: `0o755`
    /// - Floating point: `3.14`, `1.0e-10`
    ///
    /// ## Examples
    ///
    /// ```
    /// use monobase::compiler::{Lexer, TokenKind};
    ///
    /// let mut lexer = Lexer::new("42 0xFF 3.14 1.0e-10");
    ///
    /// assert!(matches!(
    ///     lexer.next_token().kind,
    ///     TokenKind::DecInteger("42")
    /// ));
    /// assert!(matches!(
    ///     lexer.next_token().kind,
    ///     TokenKind::HexInteger("0xFF")
    /// ));
    /// assert!(matches!(
    ///     lexer.next_token().kind,
    ///     TokenKind::Float("3.14")
    /// ));
    /// assert!(matches!(
    ///     lexer.next_token().kind,
    ///     TokenKind::Float("1.0e-10")
    /// ));
    /// ```
    fn number(&mut self) -> Token<'a> {
        // Handle different number formats
        if self.peek() == Some('0') {
            match self.peek_ahead(1) {
                Some('x') => return self.hex_number(),
                Some('b') => return self.binary_number(),
                Some('o') => return self.octal_number(),
                _ => {}
            }
        }

        // Decimal number
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }

        // Check for float or scientific notation
        if let Some(c) = self.peek() {
            if c == '.' {
                // Parse decimal part
                self.advance(); // Consume .
                while let Some(c) = self.peek() {
                    if c.is_ascii_digit() || c == '_' {
                        self.advance();
                    } else {
                        break;
                    }
                }
            }

            // Check for scientific notation
            if self.peek() == Some('e') || self.peek() == Some('E') {
                self.advance(); // Consume e/E
                if self.peek() == Some('+') || self.peek() == Some('-') {
                    self.advance(); // Consume sign
                }
                while let Some(c) = self.peek() {
                    if c.is_ascii_digit() || c == '_' {
                        self.advance();
                    } else {
                        break;
                    }
                }
            }
        }

        let text = &self.source[self.start..self.pos];
        if text.contains('.') || text.contains('e') || text.contains('E') {
            self.make_token(TokenKind::Float(text))
        } else {
            self.make_token(TokenKind::DecInteger(text))
        }
    }

    /// Handles string literals with escape sequences.
    ///
    /// Supports both single and double quoted strings with escapes:
    /// - `"hello world"`
    /// - `'hello world'`
    /// - `"escape\"quote"`
    /// - `'escape\'quote'`
    ///
    /// ## Examples
    ///
    /// ```
    /// use monobase::compiler::{Lexer, TokenKind};
    ///
    /// let mut lexer = Lexer::new(r#""hello\" world""#);
    ///
    /// assert!(matches!(
    ///     lexer.next_token().kind,
    ///     TokenKind::String(r#""hello\" world""#)
    /// ));
    /// ```
    fn string(&mut self) -> Token<'a> {
        let quote = self.advance().unwrap(); // Get opening quote
        while let Some(c) = self.peek() {
            if c == quote {
                self.advance(); // Consume closing quote
                break;
            }
            if c == '\\' {
                self.advance(); // Skip escape char
                self.advance(); // Skip escaped char
            } else {
                self.advance();
            }
        }
        let text = &self.source[self.start..self.pos];
        self.make_token(TokenKind::String(text))
    }

    /// Handles identifiers and keywords.
    ///
    /// Recognizes:
    /// - Plain identifiers: `name`, `count`, `_private`
    /// - Keywords are handled as identifiers and interpreted by the parser
    ///
    /// ## Examples
    ///
    /// ```
    /// use monobase::compiler::{Lexer, TokenKind};
    ///
    /// let mut lexer = Lexer::new("name _private");
    ///
    /// assert!(matches!(
    ///     lexer.next_token().kind,
    ///     TokenKind::PlainIdentifier("name")
    /// ));
    /// assert!(matches!(
    ///     lexer.next_token().kind,
    ///     TokenKind::PlainIdentifier("_private")
    /// ));
    /// ```
    fn identifier(&mut self) -> Token<'a> {
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }
        let text = &self.source[self.start..self.pos];
        self.make_token(TokenKind::PlainIdentifier(text))
    }

    /// Handles variable names starting with $.
    ///
    /// Recognizes:
    /// - Variable names: `$name`, `$_count`, `$value123`
    ///
    /// ## Examples
    ///
    /// ```
    /// use monobase::compiler::{Lexer, TokenKind};
    ///
    /// let mut lexer = Lexer::new("$name $_count");
    ///
    /// assert!(matches!(
    ///     lexer.next_token().kind,
    ///     TokenKind::Variable("$name")
    /// ));
    /// assert!(matches!(
    ///     lexer.next_token().kind,
    ///     TokenKind::Variable("$_count")
    /// ));
    /// ```
    fn variable(&mut self) -> Token<'a> {
        self.advance(); // Skip $
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }
        let text = &self.source[self.start..self.pos];
        self.make_token(TokenKind::Variable(text))
    }

    /// Get the next character without advancing
    fn peek(&self) -> Option<char> {
        self.source[self.pos..].chars().next()
    }

    /// Get character at offset without advancing
    fn peek_ahead(&self, offset: usize) -> Option<char> {
        self.source[self.pos..].chars().nth(offset)
    }

    /// Advance the lexer position
    fn advance(&mut self) -> Option<char> {
        if let Some(c) = self.source[self.pos..].chars().next() {
            self.pos += c.len_utf8();
            if c == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
            Some(c)
        } else {
            None
        }
    }

    /// Create a token with the current span
    fn make_token(&self, kind: TokenKind<'a>) -> Token<'a> {
        Token {
            span: Span {
                start: self.start,
                end: self.pos,
            },
            kind,
        }
    }

    fn skip_whitespace(&mut self) {
        while let Some(c) = self.peek() {
            match c {
                ' ' | '\t' | '\r' | '\n' => {
                    self.advance();
                }
                '-' if self.peek_ahead(1) == Some('-') => {
                    // Skip comment until end of line
                    while let Some(c) = self.peek() {
                        if c == '\n' {
                            break;
                        }
                        self.advance();
                    }
                }
                _ => break,
            }
        }
    }

    // Operator methods
    fn operator_plus(&mut self) -> Token<'a> {
        self.advance();
        if self.peek() == Some('=') {
            self.advance();
            self.make_token(TokenKind::AssignPlus)
        } else {
            self.make_token(TokenKind::Plus)
        }
    }

    fn operator_minus(&mut self) -> Token<'a> {
        self.advance();
        match self.peek() {
            Some('=') => {
                self.advance();
                self.make_token(TokenKind::AssignMinus)
            }
            Some('>') => {
                self.advance();
                if self.peek() == Some('>') {
                    self.advance();
                    self.make_token(TokenKind::MultiArrowRight)
                } else {
                    self.make_token(TokenKind::ArrowRight)
                }
            }
            _ => self.make_token(TokenKind::Minus),
        }
    }

    fn hex_number(&mut self) -> Token<'a> {
        self.advance(); // Skip 0
        self.advance(); // Skip x
        while let Some(c) = self.peek() {
            if c.is_ascii_hexdigit() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }
        let text = &self.source[self.start..self.pos];
        self.make_token(TokenKind::HexInteger(text))
    }

    fn binary_number(&mut self) -> Token<'a> {
        self.advance(); // Skip 0
        self.advance(); // Skip b
        while let Some(c) = self.peek() {
            if c == '0' || c == '1' || c == '_' {
                self.advance();
            } else {
                break;
            }
        }
        let text = &self.source[self.start..self.pos];
        self.make_token(TokenKind::BinInteger(text))
    }

    fn octal_number(&mut self) -> Token<'a> {
        self.advance(); // Skip 0
        self.advance(); // Skip o
        while let Some(c) = self.peek() {
            if ('0'..='7').contains(&c) || c == '_' {
                self.advance();
            } else {
                break;
            }
        }
        let text = &self.source[self.start..self.pos];
        self.make_token(TokenKind::OctInteger(text))
    }

    fn operator_star(&mut self) -> Token<'a> {
        self.advance();
        match self.peek() {
            Some('*') => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    self.make_token(TokenKind::AssignPow)
                } else {
                    self.make_token(TokenKind::Pow)
                }
            }
            Some('=') => {
                self.advance();
                self.make_token(TokenKind::AssignMul)
            }
            _ => self.make_token(TokenKind::Star),
        }
    }

    fn operator_slash(&mut self) -> Token<'a> {
        self.advance();
        if self.peek() == Some('=') {
            self.advance();
            self.make_token(TokenKind::AssignDiv)
        } else {
            self.make_token(TokenKind::Div)
        }
    }

    fn operator_equals(&mut self) -> Token<'a> {
        self.advance();
        if self.peek() == Some('=') {
            self.advance();
            self.make_token(TokenKind::Eq)
        } else {
            self.make_token(TokenKind::Is)
        }
    }

    fn operator_less(&mut self) -> Token<'a> {
        self.advance();
        match self.peek() {
            Some('=') => {
                self.advance();
                self.make_token(TokenKind::Lte)
            }
            Some('<') => {
                self.advance();
                match self.peek() {
                    Some('=') => {
                        self.advance();
                        self.make_token(TokenKind::AssignShl)
                    }
                    Some('-') => {
                        self.advance();
                        self.make_token(TokenKind::MultiArrowLeft)
                    }
                    _ => self.make_token(TokenKind::Shl),
                }
            }
            Some('>') => {
                self.advance();
                self.make_token(TokenKind::Similarity)
            }
            Some('-') => {
                self.advance();
                self.make_token(TokenKind::ArrowLeft)
            }
            _ => self.make_token(TokenKind::Lt),
        }
    }

    fn operator_greater(&mut self) -> Token<'a> {
        self.advance();
        match self.peek() {
            Some('=') => {
                self.advance();
                self.make_token(TokenKind::Gte)
            }
            Some('>') => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    self.make_token(TokenKind::AssignShr)
                } else {
                    self.make_token(TokenKind::Shr)
                }
            }
            _ => self.make_token(TokenKind::Gt),
        }
    }

    fn operator_amp(&mut self) -> Token<'a> {
        self.advance();
        match self.peek() {
            Some('&') => {
                self.advance();
                self.make_token(TokenKind::And)
            }
            Some('=') => {
                self.advance();
                self.make_token(TokenKind::AssignBitAnd)
            }
            _ => self.make_token(TokenKind::BitAnd),
        }
    }

    fn operator_pipe(&mut self) -> Token<'a> {
        self.advance();
        match self.peek() {
            Some('|') => {
                self.advance();
                self.make_token(TokenKind::Or)
            }
            Some('=') => {
                self.advance();
                self.make_token(TokenKind::AssignBitOr)
            }
            _ => self.make_token(TokenKind::BitOr),
        }
    }

    fn operator_caret(&mut self) -> Token<'a> {
        self.advance();
        if self.peek() == Some('=') {
            self.advance();
            self.make_token(TokenKind::AssignBitXor)
        } else {
            self.make_token(TokenKind::BitXor)
        }
    }

    fn operator_tilde(&mut self) -> Token<'a> {
        self.advance();
        if self.peek() == Some('=') {
            self.advance();
            self.make_token(TokenKind::AssignBitNot)
        } else {
            self.make_token(TokenKind::Match)
        }
    }

    fn operator_bang(&mut self) -> Token<'a> {
        self.advance();
        match self.peek() {
            Some('=') => {
                self.advance();
                self.make_token(TokenKind::IsNot)
            }
            Some('~') => {
                self.advance();
                self.make_token(TokenKind::NotMatch)
            }
            _ => self.make_token(TokenKind::Not),
        }
    }

    fn operator_question(&mut self) -> Token<'a> {
        self.advance();
        match self.peek() {
            Some('.') => {
                self.advance();
                self.make_token(TokenKind::SafeNav)
            }
            Some(':') => {
                self.advance();
                self.make_token(TokenKind::NullCoalesce)
            }
            _ => self.make_token(TokenKind::Optional),
        }
    }

    fn operator_colon(&mut self) -> Token<'a> {
        self.advance();
        if self.peek() == Some(':') {
            self.advance();
            self.make_token(TokenKind::Scope)
        } else {
            self.make_token(TokenKind::Colon)
        }
    }

    fn operator_dot(&mut self) -> Token<'a> {
        self.advance();
        // Check if followed by a digit (for float starting with dot)
        if let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                // Parse decimal part
                while let Some(c) = self.peek() {
                    if c.is_ascii_digit() || c == '_' {
                        self.advance();
                    } else {
                        break;
                    }
                }

                // Check for scientific notation
                if self.peek() == Some('e') || self.peek() == Some('E') {
                    self.advance(); // Consume e/E
                    if self.peek() == Some('+') || self.peek() == Some('-') {
                        self.advance(); // Consume sign
                    }
                    while let Some(c) = self.peek() {
                        if c.is_ascii_digit() || c == '_' {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                }

                let text = &self.source[self.start..self.pos];
                return self.make_token(TokenKind::Float(text));
            }
        }

        // Check for range operators
        match self.peek() {
            Some('.') => {
                self.advance();
                if self.peek() == Some('=') {
                    self.advance();
                    self.make_token(TokenKind::RangeInclusive)
                } else {
                    self.make_token(TokenKind::Range)
                }
            }
            _ => self.make_token(TokenKind::Dot),
        }
    }

    /// Handles escaped identifiers wrapped in backticks
    fn escaped_identifier(&mut self) -> Token<'a> {
        self.advance(); // Skip opening backtick
        while let Some(c) = self.peek() {
            if c == '`' {
                self.advance(); // Consume closing backtick
                break;
            }
            self.advance();
        }
        let text = &self.source[self.start..self.pos];
        self.make_token(TokenKind::EscapedIdentifier(text))
    }

    /// Handles byte string literals
    fn byte_string(&mut self) -> Token<'a> {
        self.advance(); // Skip 'b'
        let quote = self.advance().unwrap(); // Get opening quote
        while let Some(c) = self.peek() {
            if c == quote {
                self.advance(); // Consume closing quote
                break;
            }
            if c == '\\' {
                self.advance(); // Skip escape char
                self.advance(); // Skip escaped char
            } else {
                self.advance();
            }
        }
        let text = &self.source[self.start..self.pos];
        self.make_token(TokenKind::ByteString(text))
    }

    /// Handles regex literals with flags
    fn regex(&mut self) -> Token<'a> {
        self.advance(); // Skip first /
        self.advance(); // Skip second /

        // Parse pattern
        while let Some(c) = self.peek() {
            if c == '/' && self.peek_ahead(1) == Some('/') {
                self.advance(); // Skip first closing /
                self.advance(); // Skip second closing /
                break;
            }
            if c == '\\' {
                self.advance(); // Skip escape char
                self.advance(); // Skip escaped char
            } else {
                self.advance();
            }
        }

        // Parse flags
        while let Some(c) = self.peek() {
            match c {
                'g' | 'i' | 'm' | 's' | 'u' | 'x' => {
                    self.advance();
                }
                _ => break,
            }
        }

        let text = &self.source[self.start..self.pos];
        self.make_token(TokenKind::Regex(text))
    }

    fn operator_mul_lexer(&mut self) -> Token<'a> {
        self.advance();
        if self.peek() == Some('=') {
            self.advance();
            self.make_token(TokenKind::AssignMul)
        } else {
            self.make_token(TokenKind::Mul)
        }
    }

    fn operator_div_lexer(&mut self) -> Token<'a> {
        self.advance();
        if self.peek() == Some('=') {
            self.advance();
            self.make_token(TokenKind::AssignDiv)
        } else {
            self.make_token(TokenKind::Div)
        }
    }

    fn operator_contains_lexer(&mut self) -> Token<'a> {
        self.advance();
        self.make_token(TokenKind::Contains)
    }

    fn operator_not_contains_lexer(&mut self) -> Token<'a> {
        self.advance();
        self.make_token(TokenKind::NotContains)
    }

    fn operator_contains_none_lexer(&mut self) -> Token<'a> {
        self.advance();
        self.make_token(TokenKind::ContainsNone)
    }

    fn operator_contains_all_lexer(&mut self) -> Token<'a> {
        self.advance();
        self.make_token(TokenKind::ContainsAll)
    }

    fn operator_contains_any_lexer(&mut self) -> Token<'a> {
        self.advance();
        self.make_token(TokenKind::ContainsAny)
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_tokens(input: &str, expected: Vec<TokenKind>) {
        let mut lexer = Lexer::new(input);
        let mut tokens: Vec<TokenKind> = Vec::new();

        loop {
            let token = lexer.next_token();
            let kind = token.kind;
            tokens.push(kind.clone());
            if matches!(kind, TokenKind::Eof) {
                break;
            }
        }

        assert_eq!(tokens, expected);
    }

    #[test]
    fn test_operators() {
        // Test basic operators
        assert_tokens(
            "+ - * / % ** × ÷",
            vec![
                TokenKind::Plus,
                TokenKind::Minus,
                TokenKind::Star,
                TokenKind::Div,
                TokenKind::Mod,
                TokenKind::Pow,
                TokenKind::Mul,
                TokenKind::Div,
                TokenKind::Eof,
            ],
        );

        // Test assignment operators
        assert_tokens(
            "+= -= *= /= %= **= ×= ÷=",
            vec![
                TokenKind::AssignPlus,
                TokenKind::AssignMinus,
                TokenKind::AssignMul,
                TokenKind::AssignDiv,
                TokenKind::AssignMod,
                TokenKind::AssignPow,
                TokenKind::AssignMul,
                TokenKind::AssignDiv,
                TokenKind::Eof,
            ],
        );

        // Test comparison operators
        assert_tokens(
            "== != < <= > >= = !",
            vec![
                TokenKind::Eq,
                TokenKind::IsNot,
                TokenKind::Lt,
                TokenKind::Lte,
                TokenKind::Gt,
                TokenKind::Gte,
                TokenKind::Is,
                TokenKind::Not,
                TokenKind::Eof,
            ],
        );

        // Test bitwise operators
        assert_tokens(
            "& | ^ ~ &= |= ^= ~=",
            vec![
                TokenKind::BitAnd,
                TokenKind::BitOr,
                TokenKind::BitXor,
                TokenKind::Match,
                TokenKind::AssignBitAnd,
                TokenKind::AssignBitOr,
                TokenKind::AssignBitXor,
                TokenKind::AssignBitNot,
                TokenKind::Eof,
            ],
        );

        // Test shift operators
        assert_tokens(
            "<< >> <<= >>=",
            vec![
                TokenKind::Shl,
                TokenKind::Shr,
                TokenKind::AssignShl,
                TokenKind::AssignShr,
                TokenKind::Eof,
            ],
        );
    }

    #[test]
    fn test_arrows_and_special_operators() {
        assert_tokens(
            "-> ->> <- <<- ?. ?: <> .. ..=",
            vec![
                TokenKind::ArrowRight,
                TokenKind::MultiArrowRight,
                TokenKind::ArrowLeft,
                TokenKind::MultiArrowLeft,
                TokenKind::SafeNav,
                TokenKind::NullCoalesce,
                TokenKind::Similarity,
                TokenKind::Range,
                TokenKind::RangeInclusive,
                TokenKind::Eof,
            ],
        );
    }

    #[test]
    fn test_unicode_operators() {
        assert_tokens(
            "∋ ∌ ⊅ ⊇ ⊃",
            vec![
                TokenKind::Contains,
                TokenKind::NotContains,
                TokenKind::ContainsNone,
                TokenKind::ContainsAll,
                TokenKind::ContainsAny,
                TokenKind::Eof,
            ],
        );
    }

    #[test]
    fn test_delimiters() {
        assert_tokens(
            "( ) [ ] { } , ; :: :",
            vec![
                TokenKind::ParenOpen,
                TokenKind::ParenClose,
                TokenKind::BracketOpen,
                TokenKind::BracketClose,
                TokenKind::BraceOpen,
                TokenKind::BraceClose,
                TokenKind::Comma,
                TokenKind::Terminator,
                TokenKind::Scope,
                TokenKind::Colon,
                TokenKind::Eof,
            ],
        );
    }

    #[test]
    fn test_regex_literals() {
        assert_tokens(
            "//abc// //abc//g //abc//gi //abc//gim",
            vec![
                TokenKind::Regex("//abc//"),
                TokenKind::Regex("//abc//g"),
                TokenKind::Regex("//abc//gi"),
                TokenKind::Regex("//abc//gim"),
                TokenKind::Eof,
            ],
        );
    }

    #[test]
    fn test_escaped_identifiers() {
        assert_tokens(
            "`foo` `bar_123` `_baz`",
            vec![
                TokenKind::EscapedIdentifier("`foo`"),
                TokenKind::EscapedIdentifier("`bar_123`"),
                TokenKind::EscapedIdentifier("`_baz`"),
                TokenKind::Eof,
            ],
        );
    }

    #[test]
    fn test_byte_strings() {
        assert_tokens(
            r#"b"hello" b'world' b"escape\"quote" b'escape\'quote'"#,
            vec![
                TokenKind::ByteString(r#"b"hello""#),
                TokenKind::ByteString("b'world'"),
                TokenKind::ByteString(r#"b"escape\"quote""#),
                TokenKind::ByteString(r#"b'escape\'quote'"#),
                TokenKind::Eof,
            ],
        );
    }
}
