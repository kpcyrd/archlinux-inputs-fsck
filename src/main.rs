use archlinux_inputs_fsck::args::{Args, Scan, SubCommand};
use archlinux_inputs_fsck::errors::*;
use archlinux_inputs_fsck::fsck::Finding;
use clap::Parser;
use env_logger::Env;
use strum::VariantNames;

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
        SubCommand::Check(check) => check.run(&check).await?,
        SubCommand::Vulns(vulns) => vulns.run(&vulns.check).await?,
        SubCommand::SupportedIssues => {
            for issue in Finding::VARIANTS {
                println!("{}", issue);
            }
        }
    }

    Ok(())
}
