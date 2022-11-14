use archlinux_inputs_fsck::args::{Args, SubCommand};
use archlinux_inputs_fsck::errors::*;
use archlinux_inputs_fsck::fsck::{self, Finding, Target};
use clap::Parser;
use env_logger::Env;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::fs;
use std::path::Path;
use strum::VariantNames;
use tokio::task::JoinSet;

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

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let log_level = match (args.quiet, args.verbose) {
        (0, 0) => "info",
        (0, _) => "debug",
        (1, _) => "warn",
        (_, _) => "error",
    };
    env_logger::init_from_env(Env::default().default_filter_or(log_level));

    match args.subcommand {
        SubCommand::Check(check) => {
            let mut queue = VecDeque::new();

            for dir in &check.scan_directory {
                read_pkgs_from_dir(&mut queue, dir)
                    .context("Failed to scan directory for PKGBUILDs")?;
            }

            for pkg in check.arch_build_system {
                queue.push_back(Target::ArchBuildSystem(pkg));
            }

            for path in check.paths {
                queue.push_back(Target::BuildPath(path));
            }

            let filters = HashSet::<String>::from_iter(check.filters.into_iter());

            let mut pool = JoinSet::new();

            let concurrency = num_cpus::get() * 2;
            loop {
                while pool.len() < concurrency {
                    if let Some(target) = queue.pop_front() {
                        // pkg, work_dir
                        pool.spawn(async move {
                            info!("Checking {:?}", target.display());
                            let findings = fsck::check_pkg(&target, check.discover_sigs).await;
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
        }
        SubCommand::SupportedIssues => {
            for issue in Finding::VARIANTS {
                println!("{}", issue);
            }
        }
    }

    Ok(())
}
