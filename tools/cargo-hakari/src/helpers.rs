// Copyright (c) The cargo-guppy Contributors
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{cargo_cli::CargoCli, output::OutputOpts};
use color_eyre::{eyre::WrapErr, Result};

/// Regenerate the lockfile after dependency updates.
pub(crate) fn regenerate_lockfile(output: OutputOpts) -> Result<()> {
    // This seems to be the cheapest way to update the lockfile.
    // cargo update -p <hakari-package> can sometimes cause unnecessary index updates.
    let cargo_cli = CargoCli::new("tree", output);
    cargo_cli
        .to_expression()
        .stdout_null()
        .run()
        .wrap_err("updating Cargo.lock failed")?;
    Ok(())
}
