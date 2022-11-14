use crate::errors::*;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

pub async fn checkout_package(directory: &Path, pkgbase: &str) -> Result<PathBuf> {
    debug!("Checkout out {:?} to {:?}", pkgbase, directory);
    let cmd = Command::new("asp")
        .args(["checkout", pkgbase])
        // TODO: find a better way to make it silent without discarding stderr
        .stderr(Stdio::null())
        .current_dir(directory)
        .spawn()
        .with_context(|| anyhow!("Failed to run asp checkout {:?}", pkgbase))?;

    let out = cmd.wait_with_output().await?;
    if !out.status.success() {
        bail!(
            "Process (asp checkout {:?}) exited with error: {:?}",
            pkgbase,
            out.status
        );
    }

    Ok(directory.join(pkgbase).join("trunk"))
}
