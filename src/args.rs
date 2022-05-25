use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
pub struct Args {
    /// Optional name to operate on
    name: Option<String>,

    /*
    /// Sets a custom config file
    #[clap(short, long, parse(from_os_str), value_name = "FILE")]
    config: Option<PathBuf>,
    */
    /// Turn debugging information on
    #[clap(short, long, global=true, parse(from_occurrences))]
    pub verbose: usize,

    #[clap(subcommand)]
    pub subcommand: SubCommand,
}

#[derive(Debug, Subcommand)]
pub enum SubCommand {
    Pkgs {
        #[clap(subcommand)]
        subcommand: Pkgs,
    },
    Check {
        pkgs: Vec<String>,
        #[clap(short, long)]
        all: bool,
        #[clap(short = 'W', long)]
        work_dir: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
pub enum Pkgs {
    Ls {},
}
