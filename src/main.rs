//! Wayland compositor for efficient workflows

#![warn(missing_docs)]

use scape::args::get_global_args;

#[cfg(feature = "profile-with-tracy")]
#[global_allocator]
static GLOBAL: profiling::tracy_client::ProfiledAllocator<std::alloc::System> =
    profiling::tracy_client::ProfiledAllocator::new(std::alloc::System, 10);

#[cfg(feature = "profiling")]
fn setup_profiling() {
    #[cfg(feature = "profile-with-tracy")]
    profiling::tracy_client::Client::start();

    profiling::register_thread!("Main Thread");

    #[cfg(feature = "profile-with-puffin")]
    let _server =
        puffin_http::Server::new(&format!("0.0.0.0:{}", puffin_http::DEFAULT_PORT)).unwrap();
    #[cfg(feature = "profile-with-puffin")]
    profiling::puffin::set_scopes_on(true);
}

fn setup_logging() {
    if let Ok(env_filter) = tracing_subscriber::EnvFilter::try_from_default_env() {
        tracing_subscriber::fmt()
            .compact()
            .with_env_filter(env_filter)
            .init();
    } else {
        tracing_subscriber::fmt().compact().init();
    }
}

fn main() {
    setup_logging();
    #[cfg(feature = "profiling")]
    setup_profiling();

    let args = get_global_args();
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
