use clap::Parser;
use manualaid_cli::cli::Cli;

fn main() {
    std::process::exit(manualaid_cli::commands::run_main(Cli::parse()));
}
