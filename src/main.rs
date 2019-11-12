use std::path::PathBuf;

use structopt::StructOpt;

mod commands;

fn main() {
    let opts = Opts::from_args();

    match opts.command {
        Command::Glcm { path } => commands::glcm(path),
    }
}

#[derive(StructOpt)]
struct Opts {
    #[structopt(subcommand)]
    command: Command,
}

#[derive(StructOpt)]
enum Command {
    /// Get latest commit modifying a given file or directory
    #[structopt(name = "glcm")]
    Glcm { path: PathBuf },
}
