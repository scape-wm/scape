//! Wayland compositor for efficient workflows

#![warn(missing_docs)]

use clap::Parser;

#[cfg(feature = "profile-with-tracy")]
#[global_allocator]
static GLOBAL: profiling::tracy_client::ProfiledAllocator<std::alloc::System> =
    profiling::tracy_client::ProfiledAllocator::new(std::alloc::System, 10);

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

    #[cfg(feature = "profile-with-tracy")]
    profiling::tracy_client::Client::start();

    profiling::register_thread!("Main Thread");

    #[cfg(feature = "profile-with-puffin")]
    let _server =
        puffin_http::Server::new(&format!("0.0.0.0:{}", puffin_http::DEFAULT_PORT)).unwrap();
    #[cfg(feature = "profile-with-puffin")]
    profiling::puffin::set_scopes_on(true);

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
