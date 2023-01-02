use crate::asp;
use crate::errors::*;
use crate::fsck;
use crate::fsck::{Finding, Target};
use crate::osv;
use async_trait::async_trait;
use clap::{builder::PossibleValuesParser, ArgAction, Parser, Subcommand};
use std::collections::HashSet;
use std::collections::VecDeque;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use strum::VariantNames;
use tokio::process::Command;
use tokio::task::JoinSet;

#[derive(Debug, Parser)]
pub struct Args {
    /// Turn debugging information on
    #[arg(short, long, global = true, action(ArgAction::Count))]
    pub verbose: u8,
    /// Less verbose output
    #[arg(short, long, global = true, action(ArgAction::Count))]
    pub quiet: u8,
    #[command(subcommand)]
    pub subcommand: SubCommand,
}

#[derive(Debug, Subcommand)]
pub enum SubCommand {
    Check(Check),
    Vulns(Vulns),
    SupportedIssues,
}

#[derive(Debug, Parser, Clone)]
pub struct Check {
    pub paths: Vec<PathBuf>,
    /// Scan directory for PKGBUILDs or specify the work directory to clone packages into (eg. ./svntogit-packages)
    #[arg(short = 'W', short_alias = 'S', long, value_name = "PATH")]
    pub scan_directory: Vec<PathBuf>,
    /// Checkout PKGBUILD with asp from devtools into a temporary directory
    #[arg(short = 'B', long, value_name = "PKG_NAME")]
    pub arch_build_system: Vec<String>,
    /// Filter only for specific findings
    #[arg(long)]
    pub discover_sigs: bool,
    /// Filter only for specific findings
    #[arg(
        short,
        long = "filter",
        value_parser(PossibleValuesParser::new(Finding::VARIANTS))
    )]
    pub filters: Vec<String>,
    /// Print package names with findings to stdout
    #[arg(short, long)]
    pub report: bool,
    #[arg(short='j', long)]
    pub concurrency: Option<usize>,
}

#[derive(Debug, Parser)]
pub struct Vulns {
    #[clap(flatten)]
    pub check: Check,
}

fn read_pkgs_from_dir(out: &mut VecDeque<Target>, path: &Path) -> Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let filename = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow!("Failed to convert directory name to string"))?;
        if filename == ".git" {
            continue;
        }
        let path = entry.path().join("trunk");
        out.push_back(Target::BuildPath(path));
    }

    Ok(())
}

#[async_trait]
pub trait Scan {
    async fn scan(target: &Target, check: &Check) -> Result<Vec<Finding>>;

    async fn run(mut check: Check) -> Result<()> {
        let mut queue = VecDeque::new();

        for dir in &check.scan_directory {
            read_pkgs_from_dir(&mut queue, dir)
                .context("Failed to scan directory for PKGBUILDs")?;
        }

        for pkg in &check.arch_build_system {
            queue.push_back(Target::ArchBuildSystem(pkg.clone()));
        }

        for path in &check.paths {
            queue.push_back(Target::BuildPath(path.clone()));
        }

        let filters = HashSet::<String>::from_iter(check.filters.drain(..));

        let mut pool = JoinSet::new();

        let concurrency = check.concurrency.unwrap_or_else(|| num_cpus::get() * 2);
        loop {
            while pool.len() < concurrency {
                if let Some(target) = queue.pop_front() {
                    // pkg, work_dir
                    let check = check.clone();
                    pool.spawn(async move {
                        let findings = Self::scan(&target, &check).await;
                        (target, findings)
                    });
                } else {
                    // no more tasks to schedule
                    break;
                }
            }

            if let Some(join) = pool.join_next().await {
                let (target, findings) = join.context("Failed to join task")?;
                match findings {
                    Ok(findings) => {
                        let has_findings = Finding::audit_list(&target, &findings, &filters);

                        if check.report && has_findings {
                            println!("{}", target.display());
                        }
                    }
                    Err(err) => {
                        error!("Failed to check package: {:?} => {:#}", target, err);
                    }
                }
            } else {
                // no more tasks in pool
                break;
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Scan for Check {
    async fn scan(target: &Target, check: &Check) -> Result<Vec<Finding>> {
        info!("Checking {:?}", target.display());
        let findings = fsck::check_pkg(&target, check.discover_sigs).await?;
        Ok(findings)
    }
}

#[async_trait]
impl Scan for Vulns {
    async fn scan(target: &Target, _check: &Check) -> Result<Vec<Finding>> {
        info!("Scanning {:?}", target.display());

        let (_temp_dir, path) = match &target {
            Target::ArchBuildSystem(pkg) => {
                let tmp = tempfile::Builder::new()
                    .prefix("archlinux-inputs-fsck")
                    .tempdir()?;
                let path = asp::checkout_package(&tmp.path(), pkg).await?;
                (Some(tmp), path)
            }
            Target::BuildPath(path) => (None, PathBuf::from(path)),
        };

        let pkgbuild_path = path.join("PKGBUILD");
        if !pkgbuild_path.exists() {
            bail!("Missing PKGBUILD: {:?}", pkgbuild_path);
        }

        let mut child = Command::new("makepkg")
            .args(&["--nodeps", "--skippgpcheck", "--nobuild"])
            .current_dir(&path)
            .stdout(Stdio::null())
            .spawn()
            .context("Failed to spawn makepkg")?;

        let status = child.wait().await?;
        println!("child process exited with status: {}", status);

        let child = Command::new("osv-scanner")
            .arg("--json")
            .arg("-r")
            .arg(&path)
            .stdout(Stdio::piped())
            .spawn()
            .context("Failed to spawn osv-scanner")?;

        let output = child.wait_with_output().await?;
        let output = serde_json::from_slice::<osv::Output>(&output.stdout)?;

        let mut findings = Vec::new();
        if let Some(results) = output.results {
            for result in results {
                for packages in result.packages {
                    findings.push(Finding::SecurityAdvisory {
                        source: result.source.clone(),
                        packages,
                    });
                }
            }
        }

        Ok(findings)
    }
}
