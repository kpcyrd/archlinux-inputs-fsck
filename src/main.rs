use archlinux_inputs_fsck::args::{Args, Pkgs, SubCommand};
use archlinux_inputs_fsck::asp;
use archlinux_inputs_fsck::errors::*;
use clap::Parser;
use env_logger::Env;

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let log_level = "info";
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

            if pkgs.len() > 1 {
                bail!("Option --work-dir can only be used with a single package at a time");
            }

            for pkg in pkgs {
                info!("Checking {:?}", pkg);

                let work_dir = if let Some(work_dir) = &work_dir {
                    work_dir.clone()
                } else {
                    todo!("random work dir");
                };

                let foo = asp::checkout_package(&pkg, &work_dir).await?;
            }
        }
    }

    Ok(())
}
