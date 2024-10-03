use clap::{Args, Parser, Subcommand};

/// A Wayland compositor for efficient workflows
#[derive(Parser, Debug, Default)]
#[command(author, version, about, long_about = None)]
pub struct GlobalArgs {
    /// Use winit as render backend instead of udev
    #[arg(short, long)]
    pub winit_backend: bool,

    /// Log to file instead of standard out
    #[arg(short, long)]
    pub log_file: Option<String>,

    /// Path to lua config file
    #[arg(short, long)]
    pub config: Option<String>,

    /// Optional sub-commands to run
    #[clap(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Cli(CliArgs),
}

#[derive(Args, Debug)]
pub struct CliArgs {
    #[clap(subcommand)]
    pub cli_command: CliCommand,
}

#[derive(Clone, Debug, Subcommand)]
pub enum CliCommand {
    CloseWindow { window_name: String },
}

/// Parses and returns the command lines arguments
pub fn get_global_args() -> GlobalArgs {
    GlobalArgs::parse()
}
