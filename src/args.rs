use crate::fsck::Finding;
use clap::{ArgAction, Parser, Subcommand, builder::PossibleValuesParser};
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
    pub pkgs: Vec<String>,
    #[arg(short, long)]
    pub all: bool,
    /// Scan directories for PKGBUILDs or specify the work directory to clone packages into
    #[arg(short = 'W', long)]
    pub work_dir: Vec<PathBuf>,
    /// Filter only for specific findings
    #[arg(long)]
    pub discover_sigs: bool,
    /// Filter only for specific findings
    #[arg(short, long="filter", value_parser(PossibleValuesParser::new(Finding::VARIANTS)))]
    pub filters: Vec<String>,
    /// Print package names with findings to stdout
    #[arg(short, long)]
    pub report: bool,
}
