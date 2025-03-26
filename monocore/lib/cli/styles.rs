use clap::builder::styling::{AnsiColor, Effects, Style, Styles};
use std::fmt::Write;

//--------------------------------------------------------------------------------------------------
// Constants
//--------------------------------------------------------------------------------------------------

#[cfg(not(test))]
/// Global flag indicating whether we're in an ANSI-capable interactive terminal
static IS_ANSI_TERMINAL: std::sync::LazyLock<bool> =
    std::sync::LazyLock::new(|| monoutils::term::is_ansi_interactive_terminal());

//--------------------------------------------------------------------------------------------------
// Functions
//--------------------------------------------------------------------------------------------------

/// Returns a `Styles` object with the default styles for the CLI.
pub fn styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Yellow.on_default() | Effects::BOLD)
        .usage(AnsiColor::Yellow.on_default() | Effects::BOLD)
        .literal(AnsiColor::Blue.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::Green.on_default())
        .error(AnsiColor::Red.on_default() | Effects::BOLD)
        .valid(AnsiColor::Green.on_default() | Effects::BOLD)
        .invalid(AnsiColor::Red.on_default() | Effects::BOLD)
}

/// Helper function to apply a style to text
fn apply_style(text: String, style: &Style) -> String {
    #[cfg(not(test))]
    if !*IS_ANSI_TERMINAL {
        return text;
    }

    #[cfg(test)]
    {
        if std::env::var("TERM").unwrap_or_default() == "dumb" {
            return text;
        }
    }

    let mut styled = String::with_capacity(text.len() + 20); // Reserve extra space for ANSI codes
    let _ = write!(styled, "{}", style);
    styled.push_str(&text);
    let _ = write!(styled, "{}", style.render_reset());
    styled
}

//--------------------------------------------------------------------------------------------------
// Traits
//--------------------------------------------------------------------------------------------------

/// A trait for applying Styles defined in [`styles`] to text.
pub trait AnsiStyles {
    /// Apply header style to text
    fn header(&self) -> String;

    /// Apply usage style to text
    fn usage(&self) -> String;

    /// Apply literal style to text
    fn literal(&self) -> String;

    /// Apply placeholder style to text
    fn placeholder(&self) -> String;

    /// Apply error style to text
    fn error(&self) -> String;

    /// Apply valid style to text
    fn valid(&self) -> String;

    /// Apply invalid style to text
    fn invalid(&self) -> String;
}

//--------------------------------------------------------------------------------------------------
// Trait Implementations
//--------------------------------------------------------------------------------------------------

impl AnsiStyles for String {
    fn header(&self) -> String {
        apply_style(self.clone(), styles().get_header())
    }

    fn usage(&self) -> String {
        apply_style(self.clone(), styles().get_usage())
    }

    fn literal(&self) -> String {
        apply_style(self.clone(), styles().get_literal())
    }

    fn placeholder(&self) -> String {
        apply_style(self.clone(), styles().get_placeholder())
    }

    fn error(&self) -> String {
        apply_style(self.clone(), styles().get_error())
    }

    fn valid(&self) -> String {
        apply_style(self.clone(), styles().get_valid())
    }

    fn invalid(&self) -> String {
        apply_style(self.clone(), styles().get_invalid())
    }
}

impl AnsiStyles for &str {
    fn header(&self) -> String {
        self.to_string().header()
    }

    fn usage(&self) -> String {
        self.to_string().usage()
    }

    fn literal(&self) -> String {
        self.to_string().literal()
    }

    fn placeholder(&self) -> String {
        self.to_string().placeholder()
    }

    fn error(&self) -> String {
        self.to_string().error()
    }

    fn valid(&self) -> String {
        self.to_string().valid()
    }

    fn invalid(&self) -> String {
        self.to_string().invalid()
    }
}

//--------------------------------------------------------------------------------------------------
// Tests
//--------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use serial_test::serial;

    use super::*;

    #[test]
    #[ignore = "this test won't work correctly in cargo-nextest. run with `cargo test -- --ignored`"]
    #[serial]
    fn test_ansi_styles_string_non_interactive() {
        helper::setup_non_interactive();

        let text = String::from("test");
        assert_eq!(text.header(), "test");
        assert_eq!(text.usage(), "test");
        assert_eq!(text.literal(), "test");
        assert_eq!(text.placeholder(), "test");
        assert_eq!(text.error(), "test");
        assert_eq!(text.valid(), "test");
        assert_eq!(text.invalid(), "test");
    }

    #[test]
    #[ignore = "this test won't work correctly in cargo-nextest. run with `cargo test -- --ignored`"]
    #[serial]
    fn test_ansi_styles_str_non_interactive() {
        helper::setup_non_interactive();

        let text = "test";
        assert_eq!(text.header(), "test");
        assert_eq!(text.usage(), "test");
        assert_eq!(text.literal(), "test");
        assert_eq!(text.placeholder(), "test");
        assert_eq!(text.error(), "test");
        assert_eq!(text.valid(), "test");
        assert_eq!(text.invalid(), "test");
    }

    #[test]
    #[ignore = "this test won't work correctly in cargo-nextest. run with `cargo test -- --ignored`"]
    #[serial]
    fn test_ansi_styles_string_interactive() {
        helper::setup_interactive();

        let text = String::from("test");

        let header = text.header();
        println!("header: {}", header);
        // Check for bold and yellow separately
        assert!(header.contains("\x1b[1m"));
        assert!(header.contains("\x1b[33m"));
        assert!(header.contains("test"));
        assert!(header.contains("\x1b[0m"));

        let usage = text.usage();
        println!("usage: {}", usage);
        assert!(usage.contains("\x1b[1m"));
        assert!(usage.contains("\x1b[33m"));
        assert!(usage.contains("test"));
        assert!(usage.contains("\x1b[0m"));

        let literal = text.literal();
        println!("literal: {}", literal);
        assert!(literal.contains("\x1b[1m"));
        assert!(literal.contains("\x1b[34m"));
        assert!(literal.contains("test"));
        assert!(literal.contains("\x1b[0m"));

        let placeholder = text.placeholder();
        println!("placeholder: {}", placeholder);
        // For placeholder, no bold is expected
        assert!(placeholder.contains("\x1b[32m"));
        assert!(placeholder.contains("test"));
        assert!(placeholder.contains("\x1b[0m"));

        let error = text.error();
        println!("error: {}", error);
        assert!(error.contains("\x1b[1m"));
        assert!(error.contains("\x1b[31m"));
        assert!(error.contains("test"));
        assert!(error.contains("\x1b[0m"));

        let valid = text.valid();
        println!("valid: {}", valid);
        assert!(valid.contains("\x1b[1m"));
        assert!(valid.contains("\x1b[32m"));
        assert!(valid.contains("test"));
        assert!(valid.contains("\x1b[0m"));

        let invalid = text.invalid();
        println!("invalid: {}", invalid);
        assert!(invalid.contains("\x1b[1m"));
        assert!(invalid.contains("\x1b[31m"));
        assert!(invalid.contains("test"));
        assert!(invalid.contains("\x1b[0m"));
    }

    #[test]
    #[ignore = "this test won't work correctly in cargo-nextest. run with `cargo test -- --ignored`"]
    #[serial]
    fn test_ansi_styles_str_interactive() {
        helper::setup_interactive();

        let text = "test";

        let header = text.header();
        assert!(header.contains("\x1b[1m"));
        assert!(header.contains("\x1b[33m"));
        assert!(header.contains("test"));
        assert!(header.contains("\x1b[0m"));

        let usage = text.usage();
        assert!(usage.contains("\x1b[1m"));
        assert!(usage.contains("\x1b[33m"));
        assert!(usage.contains("test"));
        assert!(usage.contains("\x1b[0m"));

        let literal = text.literal();
        assert!(literal.contains("\x1b[1m"));
        assert!(literal.contains("\x1b[34m"));
        assert!(literal.contains("test"));
        assert!(literal.contains("\x1b[0m"));

        let placeholder = text.placeholder();
        assert!(placeholder.contains("\x1b[32m"));
        assert!(placeholder.contains("test"));
        assert!(placeholder.contains("\x1b[0m"));

        let error = text.error();
        assert!(error.contains("\x1b[1m"));
        assert!(error.contains("\x1b[31m"));
        assert!(error.contains("test"));
        assert!(error.contains("\x1b[0m"));

        let valid = text.valid();
        assert!(valid.contains("\x1b[1m"));
        assert!(valid.contains("\x1b[32m"));
        assert!(valid.contains("test"));
        assert!(valid.contains("\x1b[0m"));

        let invalid = text.invalid();
        assert!(invalid.contains("\x1b[1m"));
        assert!(invalid.contains("\x1b[31m"));
        assert!(invalid.contains("test"));
        assert!(invalid.contains("\x1b[0m"));
    }

    #[test]
    #[ignore = "this test won't work correctly in cargo-nextest. run with `cargo test -- --ignored`"]
    #[serial]
    fn test_ansi_styles_empty_string() {
        helper::setup_interactive();

        let empty = String::new();
        assert!(empty.header().ends_with("\x1b[0m"));
        assert!(empty.usage().ends_with("\x1b[0m"));
        assert!(empty.literal().ends_with("\x1b[0m"));
        assert!(empty.placeholder().ends_with("\x1b[0m"));
        assert!(empty.error().ends_with("\x1b[0m"));
        assert!(empty.valid().ends_with("\x1b[0m"));
        assert!(empty.invalid().ends_with("\x1b[0m"));

        helper::setup_non_interactive();
        assert_eq!(empty.header(), "");
        assert_eq!(empty.usage(), "");
        assert_eq!(empty.literal(), "");
        assert_eq!(empty.placeholder(), "");
        assert_eq!(empty.error(), "");
        assert_eq!(empty.valid(), "");
        assert_eq!(empty.invalid(), "");
    }

    #[test]
    #[ignore = "this test won't work correctly in cargo-nextest. run with `cargo test -- --ignored`"]
    #[serial]
    fn test_ansi_styles_unicode_string() {
        helper::setup_interactive();

        let text = "测试";
        let header = text.header();
        assert!(header.contains("测试"));
        assert!(header.starts_with("\x1b["));
        assert!(header.ends_with("\x1b[0m"));
    }
}

#[cfg(test)]
mod helper {
    use std::env;

    /// Helper function to set up a non-interactive terminal environment
    pub(super) fn setup_non_interactive() {
        env::set_var("TERM", "dumb");
    }

    /// Helper function to set up an interactive terminal environment
    pub(super) fn setup_interactive() {
        env::set_var("TERM", "xterm-256color");
    }
}
