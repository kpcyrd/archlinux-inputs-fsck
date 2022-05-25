use crate::asp;
use crate::errors::*;
use crate::makepkg;
use crate::makepkg::Source;
use std::path::{Path, PathBuf};
use std::str::FromStr;

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

#[derive(Debug, PartialEq)]
enum AuthedSource {
    File(String),
    Url(UrlSource),
    Git(GitSource),
}

impl AuthedSource {
    fn url(s: Source) -> AuthedSource {
        AuthedSource::Url(UrlSource {
            url: s.url().to_string(),
            filename: s.filename().map(String::from),
            checksums: Vec::new(),
        })
    }
}

#[derive(Debug, PartialEq)]
struct UrlSource {
    url: String,
    filename: Option<String>,
    checksums: Vec<Checksum>,
}

impl UrlSource {
    fn is_signature_file(&self) -> bool {
        let filename = if let Some(filename) = &self.filename {
            filename
        } else {
            &self.url
        };

        for ext in [".sig", ".asc", ".sign"] {
            if filename.ends_with(ext) {
                return true;
            }
        }

        false
    }
}

#[derive(Debug, PartialEq)]
enum Checksum {
    Md5(String),
    Sha1(String),
    Sha256(String),
    Sha512(String),
    B2(String),
}

impl Checksum {
    fn new(alg: &str, value: String) -> Result<Checksum> {
        Ok(match alg {
            "md5sums" => Checksum::Md5(value),
            "sha1sums" => Checksum::Sha1(value),
            "sha256sums" => Checksum::Sha256(value),
            "sha512sums" => Checksum::Sha512(value),
            "b2sums" => Checksum::B2(value),
            _ => bail!("Unknown checksum algorithm: {:?}", alg),
        })
    }
}

impl Checksum {
    fn is_checksum_securely_pinned(&self) -> bool {
        match self {
            Checksum::Md5(_) => false,
            Checksum::Sha1(_) => false,
            Checksum::Sha256(_) => true,
            Checksum::Sha512(_) => true,
            Checksum::B2(_) => true,
        }
    }
}

#[derive(Debug, PartialEq)]
struct GitSource {
    url: String,
    commit: Option<String>,
    tag: Option<String>,
    signed: bool,
}

impl GitSource {
    fn is_commit_securely_pinned(&self) -> bool {
        if let Some(commit) = &self.commit {
            commit.len() == 40
        } else {
            false
        }
    }
}

impl FromStr for GitSource {
    type Err = Error;

    fn from_str(mut s: &str) -> Result<GitSource> {
        let mut signed = false;
        let mut commit = None;
        let mut tag = None;

        if let Some((remaining, value)) = s.rsplit_once("#commit=") {
            commit = Some(value.to_string());
            s = remaining;
        }

        if let Some((remaining, value)) = s.rsplit_once("#tag=") {
            tag = Some(value.to_string());
            s = remaining;
        }

        if let Some(remaining) = s.strip_suffix("?signed") {
            signed = true;
            s = remaining;
        }

        Ok(GitSource {
            url: s.to_string(),
            commit,
            tag,
            signed,
        })
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

    let mut sources = sources.into_iter()
        .map(|source| {
            let scheme = source.scheme();
            Ok(match &scheme {
                Some("https") => AuthedSource::url(source),
                Some("http") => AuthedSource::url(source),
                Some("ftp") => AuthedSource::url(source),
                Some(scheme) if scheme.starts_with("git") => {
                    if let "git" | "git+http" = *scheme {
                        warn!("Using insecure {}:// scheme: {:?}", scheme, source);
                    }

                    AuthedSource::Git(source.url().parse()?)
                }
                Some("svn+https") => {
                    warn!("Insecure svn+https:// scheme: {:?}", source);
                    AuthedSource::url(source)
                }
                Some(scheme) => {
                    warn!("Unknown scheme: {:?}", scheme);
                    AuthedSource::url(source)
                }
                None => AuthedSource::File(source.url().to_string()),
            })
        })
        .collect::<Result<Vec<_>>>()?;

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

        for (i, sum) in sums.into_iter().enumerate() {
            if sum == "SKIP" {
                continue;
            }

            let cm = Checksum::new(alg, sum)?;
            debug!("Found checksum for #{}: {:?}", i, cm);
            if let AuthedSource::Url(source) = &mut sources[i] {
                source.checksums.push(cm);
            }
        }
    }

    for source in sources {
        debug!("source={:?}", source);
        match source {
            AuthedSource::File(_) => (),
            AuthedSource::Url(source) => {
                if source.is_signature_file() {
                    debug!("Skipping signature file: {:?}", source);
                    continue;
                }

                if !source.checksums.iter().any(|x| x.is_checksum_securely_pinned()) {
                    warn!("Url artifact is not securely pinned by checksums: {:?}", source);
                }
            }
            AuthedSource::Git(source) => {
                if !source.is_commit_securely_pinned() {
                    warn!("Git commit is not securely pinned: {:?}", source);
                }
            }
        }
    }

    let validpgpkeys = makepkg::list_variable(&path, "validpgpkeys").await?;
    if !validpgpkeys.is_empty() {
        debug!("Found validpgpkeys={:?}", validpgpkeys);
    }

    Ok(())
}
