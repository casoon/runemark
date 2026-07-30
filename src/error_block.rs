//! Structured diagnostic and error/prerequisite presentation blocks.

use crate::color::{Console, Tone};
use crate::verdict::Verdict;
use std::io::Write;

/// A structured error or prerequisite block providing actionable remedies.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ErrorBlock {
    /// Heading or summary of the prerequisite / error.
    pub heading: String,
    /// Explanation of why the error occurred.
    pub explanation: Option<String>,
    /// Step-by-step remedy or action to resolve the issue.
    pub remedy: Option<String>,
    /// Copyable shell commands or code snippets.
    pub commands: Vec<String>,
}

impl ErrorBlock {
    /// Creates a new error block with a heading.
    pub fn new(heading: impl Into<String>) -> Self {
        Self {
            heading: heading.into(),
            explanation: None,
            remedy: None,
            commands: Vec::new(),
        }
    }

    /// Sets the detailed explanation text.
    pub fn with_explanation(mut self, text: impl Into<String>) -> Self {
        self.explanation = Some(text.into());
        self
    }

    /// Sets the remedy guidance text.
    pub fn with_remedy(mut self, text: impl Into<String>) -> Self {
        self.remedy = Some(text.into());
        self
    }

    /// Adds a copyable command snippet.
    pub fn add_command(mut self, command: impl Into<String>) -> Self {
        self.commands.push(command.into());
        self
    }

    /// Writes the error block directly to a writer.
    pub fn write_to(
        &self,
        console: Console,
        writer: &mut (impl Write + ?Sized),
    ) -> std::io::Result<()> {
        Verdict::Failed.write_to(console, &self.heading, writer)?;
        writeln!(writer)?;

        if let Some(ref exp) = self.explanation {
            crate::internal::write_toned_line("  ", console, Tone::Muted, exp, writer)?;
        }

        if let Some(ref remedy) = self.remedy {
            writeln!(writer)?;
            write!(writer, "  ")?;
            console.write_paint(Tone::Warning, "Remedy:", writer)?;
            writeln!(writer, " {remedy}")?;
        }

        if !self.commands.is_empty() {
            writeln!(writer)?;
            for cmd in &self.commands {
                crate::internal::write_toned_line(
                    "  ",
                    console,
                    Tone::Info,
                    format!("$ {cmd}"),
                    writer,
                )?;
            }
        }

        Ok(())
    }

    /// Renders the error block as a formatted String.
    pub fn render(&self, console: Console) -> String {
        crate::internal::collect_to_string(|buf| self.write_to(console, buf))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::ColorMode;

    #[test]
    fn error_block_rendering() {
        let block = ErrorBlock::new("Required toolchain is unavailable")
            .with_explanation("The configured version is not installed.")
            .with_remedy("Install the required toolchain:")
            .add_command("toolchain install stable");

        let console = Console::new(ColorMode::Never, false);
        let output = block.render(console);
        assert!(output.contains("[FAIL] Required toolchain is unavailable"));
        assert!(output.contains("The configured version is not installed."));
        assert!(output.contains("Remedy: Install the required toolchain:"));
        assert!(output.contains("$ toolchain install stable"));
    }
}
