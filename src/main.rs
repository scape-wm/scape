use clap::Parser;
use slog::{o, Drain};

/// A Wayland compositor for efficient workflows
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Use winit as render backend instead of udev
    #[arg(short, long)]
    winit_backend: bool,
}

fn main() {
    let log = slog::Logger::root(
        slog_async::Async::default(slog_term::term_full().fuse()).fuse(),
        o!(),
    );

    let _guard = slog_scope::set_global_logger(log.clone());
    slog_stdlog::init().expect("Could not setup log backend");

    let args = Args::parse();

    if args.winit_backend {
        slog::info!(log, "Starting with winit backend");
        #[cfg(feature = "winit")]
        scape::winit::run_winit(log);
    } else {
        slog::info!(log, "Starting on a tty using udev");
        #[cfg(feature = "udev")]
        scape::udev::run_udev(log);
    }
}
