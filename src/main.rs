#[macro_use]
extern crate slog;

use slog::Drain;

fn main() -> anyhow::Result<()> {
    let log = slog::Logger::root(
        slog_async::Async::default(slog_term::term_full().fuse()).fuse(),
        o!(),
    );

    let _global_log_guard = slog_scope::set_global_logger(log.clone());
    slog_stdlog::init()?;

    info!(log, "Starting Scape");

    Ok(())
}
