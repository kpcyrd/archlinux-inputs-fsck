use archlinux_inputs_fsck::args::{Args, SubCommand};
use archlinux_inputs_fsck::asp;
use archlinux_inputs_fsck::errors::*;
use archlinux_inputs_fsck::fsck::{self, Finding};
use clap::Parser;
use env_logger::Env;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use tokio::task::JoinSet;

fn read_pkgs_from_dir(out: &mut VecDeque<(String, Option<PathBuf>)>, path: &Path) -> Result<()> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let filename = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow!("Failed to convert directory name to string"))?;
        if filename == ".git" {
            continue;
        }
        out.push_back((filename, Some(path.to_owned())));
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
            let mut queue = VecDeque::<(String, Option<PathBuf>)>::new();

            if check.all {
                if !check.pkgs.is_empty() {
                    bail!("Setting packages explicitly is not allowed if --all is used");
                }

                if !check.work_dir.is_empty() {
                    for work_dir in &check.work_dir {
                        read_pkgs_from_dir(&mut queue, work_dir)?;
                    }
                } else {
                    for pkg in asp::list_packages().await? {
                        queue.push_back((pkg, None));
                    }
                }
            } else {
                for pkg in check.pkgs {
                    queue.push_back((pkg, None));
                }
            }

            if queue.is_empty() {
                bail!("No packages selected");
            }

            let filters = HashSet::<String>::from_iter(check.filters.into_iter());

            let mut pool = JoinSet::new();

            let concurrency = num_cpus::get() * 2;
            loop {
                while pool.len() < concurrency {
                    if let Some((pkg, work_dir)) = queue.pop_front() {
                        pool.spawn(async move {
                            info!("Checking {:?}", pkg);
                            let findings =
                                fsck::check_pkg(&pkg, work_dir, check.discover_sigs).await;
                            (pkg, findings)
                        });
                    } else {
                        // no more tasks to schedule
                        break;
                    }
                }

                if let Some(join) = pool.join_next().await {
                    let (pkg, findings) = join.context("Failed to join task")?;
                    match findings {
                        Ok(findings) => {
                            let has_findings = Finding::audit_list(&pkg, &findings, &filters);

                            if check.report && has_findings {
                                println!("{}", pkg);
                            }
                        }
                        Err(err) => {
                            error!("Failed to check package: {:?} => {:#}", pkg, err);
                        }
                    }
                } else {
                    // no more tasks in pool
                    break;
                }
            }
        }
    }

    Ok(())
}
