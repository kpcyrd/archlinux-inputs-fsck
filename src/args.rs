use clap::{Parser, Subcommand};
use std::path::PathBuf;

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
    Check {
        pkgs: Vec<String>,
        #[clap(short, long)]
        all: bool,
        #[clap(short = 'W', long)]
        work_dir: Option<PathBuf>,
    },
}
