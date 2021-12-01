// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{
    explain::HakariExplain,
    tabular::{Row, Table},
};
use guppy::graph::{feature::StandardFeatures, DependencyDirection};
use itertools::{Itertools, Position};
use owo_colors::{OwoColorize, Style};
use std::{collections::BTreeSet, fmt};

/// A display formatter for [`HakariExplain`].
///
/// Requires the `cli-support` feature.
#[derive(Clone, Debug)]
pub struct HakariExplainDisplay<'g, 'a, 'explain> {
    explain: &'explain HakariExplain<'g, 'a>,
    styles: Box<Styles>,
}

impl<'g, 'a, 'explain> HakariExplainDisplay<'g, 'a, 'explain> {
    pub(super) fn new(explain: &'explain HakariExplain<'g, 'a>) -> Self {
        Self {
            explain,
            styles: Box::new(Styles::default()),
        }
    }

    /// Adds ANSI color codes to the output.
    pub fn colorize(&mut self) -> &mut Self {
        self.styles.colorize();
        self
    }

    fn display_platform_str(
        &self,
        platform_idx: Option<usize>,
        f: &mut fmt::Formatter,
    ) -> fmt::Result {
        match platform_idx {
            Some(idx) => {
                let triple_str = self.explain.platforms[idx].triple_str();
                write!(f, "{}", triple_str.style(self.styles.platform_style))
            }
            None => write!(f, "all"),
        }
    }
}

const DITTO_MARK: &str = "\"";

impl<'g, 'a, 'explain> fmt::Display for HakariExplainDisplay<'g, 'a, 'explain> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut table = Table::new("  {:^}  |  {:^}  {:^}  {:^}");
        // header row
        let row = Row::new()
            .with_ansi_cell("package".style(self.styles.header_style))
            .with_ansi_cell("include dev?".style(self.styles.header_style))
            .with_ansi_cell("features".style(self.styles.header_style))
            .with_ansi_cell("platform".style(self.styles.header_style));
        table.add_row(row);

        for (build_platform, explain_map) in self.explain.explain_maps() {
            for (&features, inner) in explain_map {
                let heading = format!(
                    "\non the {} platform, feature set {} was built by:\n",
                    build_platform.style(self.styles.build_platform_style),
                    FeatureDisplay { features }.style(self.styles.feature_style),
                );
                table.add_heading(heading);

                let package_set = self
                    .explain
                    .graph
                    .resolve_ids(inner.workspace_packages.keys().copied())
                    .expect("keys derived from package graph");

                // Print output in reverse dependency order within the workspace.
                for package_id in package_set.package_ids(DependencyDirection::Reverse) {
                    let inner_value = &inner.workspace_packages[package_id];

                    let name = inner_value.metadata.name();
                    let name_display = name.style(self.styles.package_name_style);
                    for (idx, (include_dev, standard_features, platform_idx)) in
                        inner_value.sets.iter().enumerate()
                    {
                        let include_dev_display =
                            include_dev.display_with(&self.styles.star_style, |include_dev, f| {
                                match include_dev {
                                    true => write!(f, "{}", "yes".style(self.styles.yes_style)),
                                    false => write!(f, "{}", "no".style(self.styles.no_style)),
                                }
                            });
                        let features_display = standard_features.display_with(
                            &self.styles.star_style,
                            |features, f| {
                                let features_str = match features {
                                    StandardFeatures::None => "none",
                                    StandardFeatures::Default => "default",
                                    StandardFeatures::All => "all",
                                };
                                write!(
                                    f,
                                    "{}",
                                    features_str.style(self.styles.standard_features_style)
                                )
                            },
                        );

                        let platform_display = platform_idx
                            .display_with(&self.styles.star_style, |&platform_idx, f| {
                                self.display_platform_str(platform_idx, f)
                            });

                        let mut row = Row::new();
                        if idx == 0 {
                            row.add_ansi_cell(&name_display);
                        } else {
                            row.add_ansi_cell(DITTO_MARK.style(self.styles.ditto_style));
                        }

                        row.add_ansi_cell(include_dev_display)
                            .add_ansi_cell(features_display)
                            .add_ansi_cell(platform_display);
                        table.add_row(row);
                    }
                }

                for (idx, platform_idx) in inner.fixup_platforms.iter().enumerate() {
                    let mut row = Row::new();
                    if idx == 0 {
                        row.add_ansi_cell("post-compute fixup");
                    } else {
                        row.add_ansi_cell(DITTO_MARK.style(self.styles.ditto_style));
                    }

                    let platform_display = platform_idx
                        .display_with(&self.styles.star_style, |&platform_idx, f| {
                            self.display_platform_str(platform_idx, f)
                        });

                    row.add_ansi_cell("-")
                        .add_ansi_cell("-")
                        .add_ansi_cell(platform_display);
                    table.add_row(row);
                }
            }
        }

        writeln!(f, "{}", table)
    }
}

#[derive(Clone, Debug, Default)]
struct Styles {
    build_platform_style: Style,
    feature_style: Style,

    header_style: Style,
    package_name_style: Style,
    ditto_style: Style,
    star_style: Style,
    yes_style: Style,
    no_style: Style,
    standard_features_style: Style,
    platform_style: Style,
}

impl Styles {
    fn colorize(&mut self) {
        self.build_platform_style = Style::new().blue().bold();
        self.feature_style = Style::new().purple().bold();

        self.header_style = Style::new().bold();
        self.package_name_style = Style::new().bold();
        self.ditto_style = Style::new().dimmed();
        self.star_style = Style::new().dimmed();
        self.yes_style = Style::new().bright_green();
        self.no_style = Style::new().bright_red();
        self.standard_features_style = Style::new().bright_blue();
        self.platform_style = Style::new().yellow();
    }
}

#[derive(Clone, Debug)]
struct FeatureDisplay<'g, 'a> {
    features: &'a BTreeSet<&'g str>,
}

impl<'g, 'a> fmt::Display for FeatureDisplay<'g, 'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.features.is_empty() {
            return write!(f, "(no features)");
        }

        for feature in self.features.iter().with_position() {
            match feature {
                Position::First(feature) | Position::Middle(feature) => {
                    write!(f, "{}, ", feature)?;
                }
                Position::Last(feature) | Position::Only(feature) => {
                    write!(f, "{}", feature)?;
                }
            }
        }
        Ok(())
    }
}
