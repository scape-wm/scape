//! Wayland compositor for efficient workflows

#![warn(missing_docs)]

use scape::{args::get_global_args, wayland, xdg::tilde_expand};
use smithay::reexports::rustix::path::Arg;
use tracing_subscriber::filter::{EnvFilter, LevelFilter};

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
    std::mem::forget(
        puffin_http::Server::new(&format!("0.0.0.0:{}", puffin_http::DEFAULT_PORT)).unwrap(),
    );
    #[cfg(feature = "profile-with-puffin")]
    profiling::puffin::set_scopes_on(true);
}

fn setup_logging(log_file: Option<&str>) {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        let builder = EnvFilter::builder();
        #[cfg(feature = "debug")]
        let builder = builder.with_default_directive(LevelFilter::DEBUG.into());
        #[cfg(not(feature = "debug"))]
        let builder = builder.with_default_directive(LevelFilter::INFO.into());
        builder.parse_lossy("")
    });

    let log_builder = tracing_subscriber::fmt()
        .compact()
        .with_env_filter(env_filter);

    if let Some(log_file) = log_file {
        log_builder
            .with_writer(
                std::fs::OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(tilde_expand(log_file.as_bytes()).as_str().unwrap())
                    .unwrap(),
            )
            .init();
    } else {
        log_builder.init();
    }
}

fn main() -> anyhow::Result<()> {
    let args = get_global_args();

    setup_logging(args.log_file.as_deref());
    #[cfg(feature = "profiling")]
    setup_profiling();

    wayland::run(&args)
}
