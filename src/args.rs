use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
pub struct Cli {
    /// Optional name to operate on
    name: Option<String>,

    /*
    /// Sets a custom config file
    #[clap(short, long, parse(from_os_str), value_name = "FILE")]
    config: Option<PathBuf>,
    */
    /// Turn debugging information on
    #[clap(short, long, parse(from_occurrences))]
    verbose: usize,

    #[clap(subcommand)]
    command: Option<SubCommand>,
}

#[derive(Debug, Subcommand)]
pub enum SubCommand {
    /// does testing things
    Test {
        /// lists test values
        #[clap(short, long)]
        list: bool,
    },
}
