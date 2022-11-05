use crate::fsck::Finding;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use strum::VariantNames;

#[derive(Debug, Parser)]
pub struct Args {
    /// Turn debugging information on
    #[clap(short, long, global = true, parse(from_occurrences))]
    pub verbose: usize,
    /// Less verbose output
    #[clap(short, long, global = true, parse(from_occurrences))]
    pub quiet: usize,
    #[clap(subcommand)]
    pub subcommand: SubCommand,
}

#[derive(Debug, Subcommand)]
pub enum SubCommand {
    Check(Check),
}

#[derive(Debug, Parser)]
pub struct Check {
    pub pkgs: Vec<String>,
    #[clap(short, long)]
    pub all: bool,
    #[clap(short = 'W', long)]
    pub work_dir: Option<PathBuf>,
    /// Filter only for specific findings
    #[clap(long)]
    pub discover_sigs: bool,
    /// Filter only for specific findings
    #[clap(short, long="filter", possible_values=Finding::VARIANTS)]
    pub filters: Vec<String>,
    /// Print package names with findings to stdout
    #[clap(short, long)]
    pub report: bool,
}
