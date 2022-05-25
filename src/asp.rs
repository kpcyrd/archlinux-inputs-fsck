use crate::errors::*;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::process::Command;

pub async fn list_packages() -> Result<Vec<String>> {
    let cmd = Command::new("asp")
        .arg("list-all")
        .stdout(Stdio::piped())
        .spawn()
        .context("Failed to run asp list-all")?;

    let out = cmd.wait_with_output().await?;
    if !out.status.success() {
        bail!("Process (asp list-all) exited with error: {:?}", out.status);
    }

    let buf = String::from_utf8(out.stdout).context("List of packages contains invalid utf8")?;
    Ok(buf.lines().map(String::from).collect())
}

pub async fn checkout_package(pkgbase: &str, directory: &Path) -> Result<PathBuf> {
    info!("Checkout out {:?} to {:?}", pkgbase, directory);
    let cmd = Command::new("asp")
        .args(&["checkout", pkgbase])
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

    Ok(directory.join(pkgbase))
}
