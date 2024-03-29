use crate::asp;
use crate::bzr::BzrSource;
use crate::errors::*;
use crate::git::GitSource;
use crate::github;
use crate::hg::HgSource;
use crate::makepkg;
use crate::makepkg::Source;
use crate::osv;
use crate::svn::SvnSource;
use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt;
use std::path::PathBuf;
use strum::{EnumVariantNames, IntoStaticStr};

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Target {
    ArchBuildSystem(String),
    BuildPath(PathBuf),
}

impl Target {
    pub fn display(&self) -> Cow<str> {
        match self {
            Target::ArchBuildSystem(pkg) => Cow::Borrowed(pkg),
            Target::BuildPath(path) => path.to_string_lossy(),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
enum AuthedSource {
    File(String),
    Url(UrlSource),
    Git(GitSource),
    Svn(SvnSource),
    Hg(HgSource),
    Bzr(BzrSource),
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
    Sha224(String),
    Sha384(String),
    B2(String),
}

impl Checksum {
    fn new(alg: &str, value: String) -> Result<Checksum> {
        Ok(match alg {
            "md5sums" => Checksum::Md5(value),
            "sha1sums" => Checksum::Sha1(value),
            "sha256sums" => Checksum::Sha256(value),
            "sha512sums" => Checksum::Sha512(value),
            "sha224sums" => Checksum::Sha224(value),
            "sha384sums" => Checksum::Sha384(value),
            "b2sums" => Checksum::B2(value),
            _ => bail!("Unknown checksum algorithm: {:?}", alg),
        })
    }

    fn is_checksum_securely_pinned(&self) -> bool {
        match self {
            Checksum::Md5(_) => false,
            Checksum::Sha1(_) => false,
            Checksum::Sha256(_) => true,
            Checksum::Sha512(_) => true,
            Checksum::Sha224(_) => true,
            Checksum::Sha384(_) => true,
            Checksum::B2(_) => true,
        }
    }
}

#[derive(IntoStaticStr, EnumVariantNames, Clone)]
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
    BzrInsecurePin(BzrSource),
    UrlArtifactInsecurePin(UrlSource),
    SecurityAdvisory {
        source: PathBuf,
        packages: osv::Packages,
    },
}

impl Finding {
    pub fn audit_list(target: &Target, findings: &[Self], filters: &HashSet<String>) -> bool {
        let mut has_findings = false;

        for finding in findings {
            let key: &'static str = finding.into();
            if filters.is_empty() || filters.contains(key) {
                warn!("{:?}: {}", target.display(), finding);
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
            Finding::BzrInsecurePin(source) => {
                write!(
                    w,
                    "bzr is never a cryptographically secure pin: {:?}",
                    source
                )
            }
            Finding::UrlArtifactInsecurePin(source) => {
                write!(
                    w,
                    "Url artifact is not securely pinned by checksums: {:?}",
                    source
                )
            }
            Finding::SecurityAdvisory { source, packages } => {
                write!(
                    w,
                    "Security advisory exists in dependency {:?} {:?} referenced by checked out source code at {:?}: ",
                    packages.package.name,
                    packages.package.version.as_deref().unwrap_or("-"),
                    source,
                )?;
                let mut first = true;
                for groups in &packages.groups {
                    for group in groups {
                        for id in &group.ids {
                            if first {
                                first = false;
                            } else {
                                write!(w, ", ")?;
                            }
                            write!(w, "https://osv.dev/vulnerability/{}", id)?;
                        }
                    }
                }
                Ok(())
            }
        }
    }
}

pub async fn check_pkg(target: &Target, discover_sigs: bool) -> Result<Vec<Finding>> {
    let client = reqwest::Client::builder()
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION"),
        ))
        .build()?;

    let (_temp_dir, path) = match &target {
        Target::ArchBuildSystem(pkg) => {
            let tmp = tempfile::Builder::new()
                .prefix("archlinux-inputs-fsck")
                .tempdir()?;
            let path = asp::checkout_package(tmp.path(), pkg).await?;
            (Some(tmp), path)
        }
        Target::BuildPath(path) => (None, PathBuf::from(path)),
    };

    let pkgbuild_path = path.join("PKGBUILD");
    if !pkgbuild_path.exists() {
        bail!("Missing PKGBUILD: {:?}", pkgbuild_path);
    }

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
                        // Mark all insecure ones
                        findings.push(Finding::InsecureScheme {
                            scheme: scheme.to_string(),
                            source: source.clone(),
                        });
                    } else if !matches!(*scheme, "git+https") {
                        // Mark all that aren't known as secure as `unknown`
                        findings.push(Finding::UnknownScheme((scheme.to_string(), source.clone())));
                    }

                    AuthedSource::Git(source.url().parse()?)
                }
                Some(scheme) if scheme.starts_with("svn") => {
                    if let "svn" | "svn+http" = *scheme {
                        // Mark all insecure ones
                        findings.push(Finding::InsecureScheme {
                            scheme: scheme.to_string(),
                            source: source.clone(),
                        });
                    } else if !matches!(*scheme, "svn+https") {
                        // Mark all that aren't known as secure as `unknown`
                        findings.push(Finding::UnknownScheme((scheme.to_string(), source.clone())));
                    }

                    AuthedSource::Svn(source.url().parse()?)
                }
                Some(scheme) if scheme.starts_with("hg") => {
                    if *scheme == "hg+http" {
                        // Mark all insecure ones
                        findings.push(Finding::InsecureScheme {
                            scheme: scheme.to_string(),
                            source: source.clone(),
                        });
                    } else if !matches!(*scheme, "hg+https") {
                        // Mark all that aren't known as secure as `unknown`
                        findings.push(Finding::UnknownScheme((scheme.to_string(), source.clone())));
                    }

                    AuthedSource::Hg(source.url().parse()?)
                }
                Some(scheme) if scheme.starts_with("bzr") => {
                    if *scheme == "bzr+http" {
                        // Mark all insecure ones
                        findings.push(Finding::InsecureScheme {
                            scheme: scheme.to_string(),
                            source: source.clone(),
                        });
                    } else if !matches!(*scheme, "bzr+https") {
                        // Mark all that aren't known as secure as `unknown`
                        findings.push(Finding::UnknownScheme((scheme.to_string(), source.clone())));
                    }

                    AuthedSource::Bzr(source.url().parse()?)
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
            AuthedSource::Bzr(source) => {
                findings.push(Finding::BzrInsecurePin(source));
            }
        }
    }

    let validpgpkeys = makepkg::list_variable(&path, "validpgpkeys").await?;
    if !validpgpkeys.is_empty() {
        debug!("Found validpgpkeys={:?}", validpgpkeys);
    }

    Ok(findings)
}
