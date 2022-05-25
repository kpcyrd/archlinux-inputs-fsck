use crate::errors::*;
use lazy_static::lazy_static;
use std::collections::HashSet;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

pub const SUPPORTED_ALGS: &[&str] = &["sha256sums", "sha512sums", "b2sums", "md5sums", "sha1sums"];

lazy_static! {
    pub static ref INSECURE_ALGS: HashSet<&'static str> =
        ["md5sums", "sha1sums"].into_iter().collect();
}

#[derive(Debug, PartialEq)]
pub enum Source {
    Url(String),
    UrlWithFilename((String, String)),
}

impl Source {
    pub fn url(&self) -> &str {
        match self {
            Source::Url(url) => url,
            Source::UrlWithFilename((url, _file)) => url,
        }
    }

    pub fn scheme(&self) -> Option<&str> {
        self.url().split_once("://").map(|x| x.0)
    }
}

async fn exec_sh(folder: &Path, cmd: &str) -> Result<Vec<String>> {
    let child = Command::new("bash")
        .arg("-c")
        .arg(format!("source ./PKGBUILD;{}", cmd))
        .stdout(Stdio::piped())
        .current_dir(folder)
        .spawn()
        .context("Failed to run bash")?;

    let out = child.wait_with_output().await?;
    if !out.status.success() {
        bail!(
            "Process (bash, {:?}) exited with error: {:?}",
            cmd,
            out.status
        );
    }

    let buf = String::from_utf8(out.stdout).context("Shell output contains invalid utf8")?;
    Ok(buf.lines().map(String::from).collect())
}

pub async fn list_variable(folder: &Path, var: &str) -> Result<Vec<String>> {
    exec_sh(
        folder,
        &format!("for x in ${{{}[@]}}; do echo \"$x\"; done", var),
    )
    .await
}

pub async fn list_sources(folder: &Path) -> Result<Vec<Source>> {
    let sources = list_variable(folder, "source").await?;
    let sources = sources
        .into_iter()
        .map(|line| {
            if let Some((file, url)) = line.split_once("::") {
                Source::UrlWithFilename((url.to_string(), file.to_string()))
            } else {
                Source::Url(line)
            }
        })
        .collect();
    Ok(sources)
}
