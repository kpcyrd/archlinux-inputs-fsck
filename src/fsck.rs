use crate::asp;
use crate::errors::*;
use crate::github;
use crate::makepkg;
use crate::makepkg::Source;
use std::path::PathBuf;
use std::str::FromStr;

enum WorkDir {
    Random(tempfile::TempDir),
    Explicit(PathBuf),
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
            is_git_object_hash(commit)
        } else if let Some(tag) = &self.tag {
            is_git_object_hash(tag)
        } else {
            false
        }
    }
}

fn is_git_object_hash(name: &str) -> bool {
    name.len() == 40 && name.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

impl FromStr for GitSource {
    type Err = Error;

    fn from_str(mut s: &str) -> Result<GitSource> {
        let mut signed = false;
        let mut commit = None;
        let mut tag = None;

        if let Some(remaining) = s.strip_suffix("?signed") {
            signed = true;
            s = remaining;
        }

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

pub async fn check_pkg(pkg: &str, work_dir: Option<PathBuf>, discover_sigs: bool) -> Result<()> {
    let client = reqwest::Client::builder()
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION"),
        ))
        .build()?;

    let work_dir = if let Some(work_dir) = &work_dir {
        WorkDir::Explicit(work_dir.clone())
    } else {
        let tmp = tempfile::Builder::new()
            .prefix("archlinux-inputs-fsck")
            .tempdir()?;
        WorkDir::Random(tmp)
    };

    let path = match &work_dir {
        WorkDir::Explicit(root) => {
            let mut path = root.clone();
            path.push(pkg);
            path.push("trunk");
            path
        }
        WorkDir::Random(tmp) => {
            let mut path = asp::checkout_package(pkg, tmp.path()).await?;
            path.push("trunk");
            path
        }
    };

    let sources = makepkg::list_sources(&path).await?;
    debug!("Found sources: {:?}", sources);

    let mut findings = Vec::new();

    let mut sources = sources
        .into_iter()
        .map(|source| {
            let scheme = source.scheme();
            Ok(match &scheme {
                Some("https") => AuthedSource::url(source),
                Some("http") => AuthedSource::url(source),
                Some("ftp") => AuthedSource::url(source),
                Some(scheme) if scheme.starts_with("git") => {
                    if let "git" | "git+http" = *scheme {
                        findings.push(format!("Using insecure {}:// scheme: {:?}", scheme, source));
                        findings.push(format!("Using insecure {}:// scheme: {:?}", scheme, source));
                    }

                    AuthedSource::Git(source.url().parse()?)
                }
                Some("svn+https") => {
                    findings.push(format!("Insecure svn+https:// scheme: {:?}", source));
                    AuthedSource::url(source)
                }
                Some(scheme) => {
                    findings.push(format!("Unknown scheme: {:?}", scheme));
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
            findings.push(format!(
                "Number of checksums doesn't match number of sources (sources={}, {}={})",
                sources.len(),
                alg,
                sums.len()
            ));
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

    // if an upstream project has submodules it's normal for them to be listed
    // in source= without pinning them by commit. As long as the primary repo
    // is securely pinned it's fine, but there's no reliable way to determine which
    // one is the primary one. So we just assume if any is pinned it's a-okay.
    let has_any_secure_git_sources = sources.iter().any(|source| match source {
        AuthedSource::Git(source) => source.is_commit_securely_pinned(),
        _ => false,
    });

    for source in sources {
        debug!("source={:?}", source);
        match source {
            AuthedSource::File(_) => (),
            AuthedSource::Url(source) => {
                if source.is_signature_file() {
                    debug!("Skipping signature file: {:?}", source);
                    continue;
                }

                if !source
                    .checksums
                    .iter()
                    .any(|x| x.is_checksum_securely_pinned())
                {
                    findings.push(format!(
                        "Url artifact is not securely pinned by checksums: {:?}",
                        source
                    ));
                }

                /*
                let re =
                    Regex::new(r"^https://gitlab.com/[^/]+/([^/]+)/-/archive/(.+)/[^/]+.tar.gz$")?;
                */

                if discover_sigs {
                    if let Some(upstream) = github::detect_signed_tag_from_url(&source.url)? {
                        let tag = github::fetch_tag(
                            &client,
                            &upstream.owner,
                            &upstream.name,
                            &upstream.tag,
                        )
                        .await?;
                        if tag.object.r#type == "tag" {
                            info!(
                                "✨ There's likely a signed tag here we could use: {:?}",
                                tag
                            );
                        }
                    }
                }
            }
            AuthedSource::Git(source) => {
                if !has_any_secure_git_sources && !source.is_commit_securely_pinned() {
                    findings.push(format!("Git commit is not securely pinned: {:?}", source));
                }
            }
        }
    }

    let validpgpkeys = makepkg::list_variable(&path, "validpgpkeys").await?;
    if !validpgpkeys.is_empty() {
        debug!("Found validpgpkeys={:?}", validpgpkeys);
    }

    for finding in findings {
        warn!("{:?}: {}", pkg, finding);
    }

    Ok(())
}
