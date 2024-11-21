//! Compiler module for the monoql language.
//!
//! This module provides the lexical analysis, parsing, and compilation infrastructure
//! for the monoql query language. It includes:
//!
//! - A lexer that tokenizes source code
//! - A parser that builds an AST from tokens
//! - Token definitions and span tracking
//!
//! # Example
//!
//! ```
//! use monobase::compiler::Lexer;
//!
//! let mut lexer = Lexer::new("select * from users");
//! while let Some(token) = lexer.next_token() {
//!     println!("{:?}", token);
//! }
//! ```

mod ast;
mod lexer;
mod parser;
mod span;

//--------------------------------------------------------------------------------------------------
// Exports
//--------------------------------------------------------------------------------------------------

pub use ast::*;
pub use lexer::*;
pub use parser::*;
pub use span::*;
