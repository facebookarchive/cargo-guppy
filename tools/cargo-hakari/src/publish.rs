// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{cargo_cli::CargoCli, helpers::regenerate_lockfile, output::OutputOpts};
use color_eyre::{eyre::WrapErr, Result};
use colored::Colorize;
use guppy::graph::PackageMetadata;
use hakari::HakariBuilder;
use log::{error, info};

pub(crate) fn publish_hakari(
    package_name: &str,
    builder: HakariBuilder<'_>,
    pass_through: &[String],
    output: OutputOpts,
) -> Result<()> {
    let hakari_package = builder
        .hakari_package()
        .expect("hakari-package must be specified in hakari.toml");
    let workspace = builder.graph().workspace();
    let package = workspace.member_by_name(package_name)?;

    // Remove the workspace-hack dependency from the package if it isn't published as open source.
    let mut remove_dep = if hakari_package.publish().is_never() {
        TempRemoveDep::new(builder, package, output)?
    } else {
        info!(
            "not removing dependency to {} because it is marked as published (publish != false)",
            hakari_package.name().bold()
        );
        TempRemoveDep::none()
    };

    let mut cargo_cli = CargoCli::new("publish", output);
    cargo_cli.add_args(pass_through.iter().map(|arg| arg.as_str()));
    // Also set --allow-dirty because we make some changes to the working directory.
    // TODO: is there a better way to handle this?
    if !remove_dep.is_none() {
        cargo_cli.add_arg("--allow-dirty");
    }

    let workspace_dir = package
        .source()
        .workspace_path()
        .expect("package is in workspace");
    let abs_path = workspace.root().join(workspace_dir);

    let all_args = cargo_cli.all_args().join(" ");

    info!("{} {}\n---", "executing".bold(), all_args);
    let expression = cargo_cli.to_expression().dir(&abs_path);

    match expression.run() {
        Ok(_) => remove_dep.finish(true),
        Err(err) => {
            remove_dep.finish(false)?;
            Err(err).wrap_err_with(|| format!("`{}` failed", all_args))
        }
    }
}

/// RAII guard to ensure packages are re-added after being published.
#[derive(Debug)]
struct TempRemoveDep<'g> {
    inner: Option<TempRemoveDepInner<'g>>,
}

impl<'g> TempRemoveDep<'g> {
    fn new(
        builder: HakariBuilder<'g>,
        package: PackageMetadata<'g>,
        output: OutputOpts,
    ) -> Result<Self> {
        let hakari_package = builder
            .hakari_package()
            .expect("hakari-package must be specified in hakari.toml");
        let package_set = package.to_package_set();
        let remove_ops = builder
            .remove_dep_ops(&package_set, false)
            .expect("hakari-package must be specified in hakari.toml");
        let inner = if remove_ops.is_empty() {
            info!(
                "dependency from {} to {} not present",
                package.name().bold(),
                hakari_package.name().bold()
            );
            None
        } else {
            info!(
                "removing dependency from {} to {}",
                package.name().bold(),
                hakari_package.name().bold()
            );
            remove_ops
                .apply()
                .wrap_err_with(|| format!("error removing dependency from {}", package.name()))?;
            Some(TempRemoveDepInner {
                builder,
                package,
                output,
            })
        };

        Ok(Self { inner })
    }

    fn none() -> Self {
        Self { inner: None }
    }

    fn is_none(&self) -> bool {
        self.inner.is_none()
    }

    fn finish(&mut self, success: bool) -> Result<()> {
        match self.inner.take() {
            Some(inner) => inner.finish(success),
            None => {
                // No operations need to be performed or `finish` was already called.
                Ok(())
            }
        }
    }
}

impl<'g> Drop for TempRemoveDep<'g> {
    fn drop(&mut self) {
        // Ignore errors in this impl.
        let _ = self.finish(false);
    }
}

#[derive(Debug)]
struct TempRemoveDepInner<'g> {
    builder: HakariBuilder<'g>,
    package: PackageMetadata<'g>,
    output: OutputOpts,
}

impl<'g> TempRemoveDepInner<'g> {
    fn finish(self, success: bool) -> Result<()> {
        let package_set = self.package.to_package_set();
        let add_ops = self
            .builder
            .add_dep_ops(&package_set, true)
            .expect("hakari-package must be specified in hakari.toml");

        if success {
            info!(
                "re-adding dependency from {} to {}",
                self.package.name().bold(),
                self.builder.hakari_package().unwrap().name().bold(),
            );
        } else {
            eprintln!("---");
            error!("execution failed, rolling back changes");
        }

        add_ops.apply()?;
        regenerate_lockfile(self.output)?;
        Ok(())
    }
}
