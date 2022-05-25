use archlinux_inputs_fsck::args::{Args, Pkgs, SubCommand};
use archlinux_inputs_fsck::asp;
use archlinux_inputs_fsck::errors::*;
use archlinux_inputs_fsck::fsck;
use clap::Parser;
use env_logger::Env;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let log_level = match args.verbose {
        0 => "info",
        _ => "debug",
    };
    env_logger::init_from_env(Env::default().default_filter_or(log_level));

    match args.subcommand {
        SubCommand::Pkgs { subcommand } => match subcommand {
            Pkgs::Ls { .. } => {
                let pkgs = asp::list_packages().await?;
                for pkg in pkgs {
                    println!("{}", pkg);
                }
            }
        },
        SubCommand::Check {
            pkgs,
            all,
            work_dir,
        } => {
            let pkgs = if all {
                if !pkgs.is_empty() {
                    bail!("Setting packages explicitly is not allowed if --all is used");
                }
                asp::list_packages().await?
            } else {
                pkgs
            };

            if pkgs.len() > 1 && work_dir.is_some() {
                bail!("Option --work-dir can only be used with a single package at a time");
            }

            for pkg in pkgs {
                info!("Checking {:?}", pkg);

                if let Err(err) = fsck::check_pkg(&pkg, work_dir.clone()).await {
                    error!("Failed to check package: {:?} => {:#}", pkg, err);
                }
            }
        }
    }

    Ok(())
}
