use clap::Parser;

/// A Wayland compositor for efficient workflows
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Use winit as render backend instead of udev
    #[arg(short, long)]
    winit_backend: bool,
}

fn main() {
    if let Ok(env_filter) = tracing_subscriber::EnvFilter::try_from_default_env() {
        tracing_subscriber::fmt()
            .compact()
            .with_env_filter(env_filter)
            .init();
    } else {
        tracing_subscriber::fmt().compact().init();
    }

    let args = Args::parse();

    if args.winit_backend {
        tracing::info!("Starting with winit backend");
        #[cfg(feature = "winit")]
        scape::winit::run_winit();
    } else {
        tracing::info!("Starting on a tty using udev");
        #[cfg(feature = "udev")]
        scape::udev::run_udev();
    }
}
