use crate::compiler::Lexer;

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

/// Parser is responsible for converting a stream of tokens into an Abstract Syntax Tree (AST).
///
/// The parser takes tokens from a lexer and constructs a structured representation of the program
/// following the language's grammar rules. It performs syntactic analysis to ensure the code follows
/// the correct structure and produces meaningful error messages for syntax errors.
pub struct Parser<'a> {
    /// The lexer that provides the tokens to be parsed.
    _lexer: Lexer<'a>,
}
