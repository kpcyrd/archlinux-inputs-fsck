use crate::asp;
use crate::errors::*;
use crate::makepkg;
use std::path::{Path, PathBuf};

enum WorkDir {
    Random(tempfile::TempDir),
    Explicit(PathBuf),
}

impl WorkDir {
    fn path(&self) -> &Path {
        match self {
            WorkDir::Random(tmp) => tmp.path(),
            WorkDir::Explicit(path) => path.as_ref(),
        }
    }
}

pub async fn check_pkg(pkg: &str, work_dir: Option<PathBuf>) -> Result<()> {
    let work_dir = if let Some(work_dir) = &work_dir {
        WorkDir::Explicit(work_dir.clone())
    } else {
        let tmp = tempfile::Builder::new()
            .prefix("archlinux-inputs-fsck")
            .tempdir()?;
        WorkDir::Random(tmp)
    };

    let mut path = asp::checkout_package(pkg, work_dir.path()).await?;
    path.push("trunk");

    let sources = makepkg::list_sources(&path).await?;
    debug!("Found sources: {:?}", sources);

    for source in &sources {
        match source.scheme() {
            Some("https") => (),
            Some("http") => (),
            Some("ftp") => (),
            Some("git") => warn!("Using insecure git:// scheme: {:?}", source),
            Some("git+http") => warn!("Using insecure git+http:// scheme: {:?}", source),
            Some("git+https") => (),
            Some("svn+https") => warn!("Insecure svn+https:// scheme: {:?}", source),
            Some(scheme) => warn!("Unknown scheme: {:?}", scheme),
            None => (),
        }
    }

    let mut has_secure_hashes = false;
    for alg in makepkg::SUPPORTED_ALGS {
        let sums = makepkg::list_variable(&path, alg).await?;
        if sums.is_empty() {
            continue;
        }

        debug!("Found checksums ({}): {:?}", alg, sums);

        if sources.len() != sums.len() {
            warn!(
                "Number of checksums doesn't match number of sources (sources={}, {}={})",
                sources.len(),
                alg,
                sums.len()
            );
        }

        if makepkg::INSECURE_ALGS.contains(alg) {
            debug!("PKGBUILD is using insecure hash algorithm: {:?}", alg);
        } else {
            has_secure_hashes = true;
        }
    }

    if !has_secure_hashes {
        warn!("PKGBUILD has no secure hashes");
    }

    let validpgpkeys = makepkg::list_variable(&path, "validpgpkeys").await?;
    if !validpgpkeys.is_empty() {
        debug!("Found validpgpkeys={:?}", validpgpkeys);
    }

    Ok(())
}
