use slog::{crit, o, Drain};

fn main() {
    let log = if std::env::var("SCAPE_MUTEX_LOG").is_ok() {
        slog::Logger::root(
            std::sync::Mutex::new(slog_term::term_full().fuse()).fuse(),
            o!(),
        )
    } else {
        slog::Logger::root(
            slog_async::Async::default(slog_term::term_full().fuse()).fuse(),
            o!(),
        )
    };

    let _guard = slog_scope::set_global_logger(log.clone());
    slog_stdlog::init().expect("Could not setup log backend");

    let arg = ::std::env::args().nth(1);
    match arg.as_ref().map(|s| &s[..]) {
        #[cfg(feature = "winit")]
        Some("--winit") => {
            slog::info!(log, "Starting anvil with winit backend");
            scape::winit::run_winit(log);
        }
        #[cfg(feature = "udev")]
        Some("--tty-udev") => {
            slog::info!(log, "Starting anvil on a tty using udev");
            scape::udev::run_udev(log);
        }
        Some(arg) => {
            crit!(log, "Unknown arg: {}", arg);
        }
        None => {
            #[cfg(feature = "udev")]
            {
                slog::info!(log, "Starting anvil on a tty using udev");
                scape::udev::run_udev(log);
            }
        }
    }
}
