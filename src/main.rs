use archlinux_inputs_fsck::args::{Args, SubCommand};
use archlinux_inputs_fsck::asp;
use archlinux_inputs_fsck::errors::*;
use archlinux_inputs_fsck::fsck;
use clap::Parser;
use env_logger::Env;
use std::fs;
use std::path::Path;

fn read_pkgs_from_dir(path: &Path) -> Result<Vec<String>> {
    let mut pkgs = Vec::new();

    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let filename = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow!("Failed to convert directory name to string"))?;
        if filename == ".git" {
            continue;
        }
        pkgs.push(filename);
    }

    Ok(pkgs)
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
            let pkgs = if check.all {
                if !check.pkgs.is_empty() {
                    bail!("Setting packages explicitly is not allowed if --all is used");
                }

                if let Some(work_dir) = &check.work_dir {
                    read_pkgs_from_dir(work_dir)?
                } else {
                    asp::list_packages().await?
                }
            } else {
                check.pkgs
            };

            if pkgs.is_empty() {
                bail!("No packages selected");
            }

            for pkg in pkgs {
                info!("Checking {:?}", pkg);

                if let Err(err) =
                    fsck::check_pkg(&pkg, check.work_dir.clone(), check.discover_sigs).await
                {
                    error!("Failed to check package: {:?} => {:#}", pkg, err);
                }
            }
        }
    }

    Ok(())
}
