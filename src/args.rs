use crate::fsck::Finding;
use clap::{builder::PossibleValuesParser, ArgAction, Parser, Subcommand};
use std::path::PathBuf;
use strum::VariantNames;

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
    SupportedIssues,
}

#[derive(Debug, Parser)]
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
}
