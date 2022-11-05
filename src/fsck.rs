use crate::asp;
use crate::errors::*;
use crate::git::GitSource;
use crate::github;
use crate::hg::HgSource;
use crate::makepkg;
use crate::makepkg::Source;
use crate::svn::SvnSource;
use std::collections::HashSet;
use std::fmt;
use std::path::PathBuf;
use strum::{EnumVariantNames, IntoStaticStr};

enum WorkDir {
    Random(tempfile::TempDir),
    Explicit(PathBuf),
}

#[derive(Debug, PartialEq)]
enum AuthedSource {
    File(String),
    Url(UrlSource),
    Git(GitSource),
    Svn(SvnSource),
    Hg(HgSource),
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

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct UrlSource {
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

#[derive(Debug, PartialEq, Eq, Clone)]
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

#[derive(IntoStaticStr, EnumVariantNames)]
#[strum(serialize_all = "kebab_case")]
pub enum Finding {
    InsecureScheme {
        scheme: String,
        source: Source,
    },
    UnknownScheme((String, Source)),
    WrongNumberOfChecksums {
        sources: usize,
        alg: &'static str,
        sums: usize,
    },
    GitCommitInsecurePin(GitSource),
    SvnInsecurePin(SvnSource),
    HgRevisionInsecurePin(HgSource),
    UrlArtifactInsecurePin(UrlSource),
}

impl Finding {
    pub fn audit_list(pkg: &str, findings: &[Self], filters: &HashSet<String>) -> bool {
        let mut has_findings = false;

        for finding in findings {
            let key: &'static str = finding.into();
            if filters.is_empty() || filters.contains(key) {
                warn!("{:?}: {}", pkg, finding);
                has_findings = true;
            }
        }

        has_findings
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, w: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Finding::InsecureScheme { scheme, source } => {
                write!(w, "Using insecure {}:// scheme: {:?}", scheme, source)
            }
            Finding::UnknownScheme((scheme, source)) => {
                write!(w, "Unknown scheme {:?}: {:?}", scheme, source)
            }
            Finding::WrongNumberOfChecksums { sources, alg, sums } => {
                write!(
                    w,
                    "Number of checksums doesn't match number of sources (sources={}, {}={})",
                    sources, alg, sums,
                )
            }
            Finding::GitCommitInsecurePin(source) => {
                write!(w, "Git commit is not securely pinned: {:?}", source)
            }
            Finding::SvnInsecurePin(source) => {
                write!(
                    w,
                    "svn is never a cryptographically secure pin: {:?}",
                    source
                )
            }
            Finding::HgRevisionInsecurePin(source) => {
                write!(w, "Hg revision is not securely pinned: {:?}", source)
            }
            Finding::UrlArtifactInsecurePin(source) => {
                write!(
                    w,
                    "Url artifact is not securely pinned by checksums: {:?}",
                    source
                )
            }
        }
    }
}

pub async fn check_pkg(
    pkg: &str,
    work_dir: Option<PathBuf>,
    discover_sigs: bool,
) -> Result<Vec<Finding>> {
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
                    if let "git" | "git+http" | "git+git" = *scheme {
                        findings.push(Finding::InsecureScheme {
                            scheme: scheme.to_string(),
                            source: source.clone(),
                        });
                    } else if let "git+https" = *scheme {
                        // these are fine
                    } else {
                        findings.push(Finding::UnknownScheme((scheme.to_string(), source.clone())));
                    }

                    AuthedSource::Git(source.url().parse()?)
                }
                Some(scheme) if scheme.starts_with("svn") => {
                    // TODO: check there are more insecure schemes
                    if *scheme == "svn+http" {
                        findings.push(Finding::InsecureScheme {
                            scheme: scheme.to_string(),
                            source: source.clone(),
                        });
                    } else if let "svn+https" = *scheme {
                        // these are fine
                    } else {
                        findings.push(Finding::UnknownScheme((scheme.to_string(), source.clone())));
                    }
                    AuthedSource::Svn(source.url().parse()?)
                }
                Some(scheme) if scheme.starts_with("hg") => {
                    // TODO: check there are more insecure schemes
                    if *scheme == "hg+http" {
                        findings.push(Finding::InsecureScheme {
                            scheme: scheme.to_string(),
                            source: source.clone(),
                        });
                    } else if let "hg+https" = *scheme {
                        // these are fine
                    } else {
                        findings.push(Finding::UnknownScheme((scheme.to_string(), source.clone())));
                    }
                    AuthedSource::Hg(source.url().parse()?)
                }
                Some(scheme) => {
                    findings.push(Finding::UnknownScheme((scheme.to_string(), source.clone())));
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
            findings.push(Finding::WrongNumberOfChecksums {
                sources: sources.len(),
                alg,
                sums: sums.len(),
            });
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
                    findings.push(Finding::UrlArtifactInsecurePin(source.clone()));
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
                    findings.push(Finding::GitCommitInsecurePin(source));
                }
            }
            AuthedSource::Svn(source) => {
                findings.push(Finding::SvnInsecurePin(source));
            }
            AuthedSource::Hg(source) => {
                if !source.is_revision_securely_pinned() {
                    findings.push(Finding::HgRevisionInsecurePin(source));
                }
            }
        }
    }

    let validpgpkeys = makepkg::list_variable(&path, "validpgpkeys").await?;
    if !validpgpkeys.is_empty() {
        debug!("Found validpgpkeys={:?}", validpgpkeys);
    }

    Ok(findings)
}
