// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::verify::VerifyErrors;
use indenter::indented;
use owo_colors::{OwoColorize, Style};
use std::fmt::{self, Write};

/// A display formatter for [`VerifyErrors`].
///
/// Requires the `cli-support` feature.
#[derive(Clone, Debug)]
pub struct VerifyErrorsDisplay<'g, 'verify> {
    verify: &'verify VerifyErrors<'g>,
    styles: Styles,
    color: bool,
}

impl<'g, 'verify> VerifyErrorsDisplay<'g, 'verify> {
    pub(super) fn new(verify: &'verify VerifyErrors<'g>) -> Self {
        Self {
            verify,
            styles: Styles::default(),
            color: false,
        }
    }

    /// Adds ANSI color codes to the output.
    pub fn color(&mut self) -> &mut Self {
        self.styles.colorize();
        self.color = true;
        self
    }
}

impl<'g, 'verify> fmt::Display for VerifyErrorsDisplay<'g, 'verify> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for explain in self.verify.errors() {
            writeln!(
                f,
                "for dependency {}:\n",
                explain
                    .dependency()
                    .id()
                    .style(self.styles.dependency_id_style)
            )?;
            let mut display = explain.display();
            if self.color {
                display.colorize();
            }
            write!(indented(f).with_str("  "), "{}", display)?;
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
struct Styles {
    dependency_id_style: Style,
}

impl Styles {
    fn colorize(&mut self) {
        self.dependency_id_style = Style::new().bright_magenta();
    }
}
