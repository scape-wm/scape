use calloop::EventLoop;
use tracing::{error, info};
use zbus::Connection;

// pub mod org_gnome_mutter_screencast;
pub mod portals;

struct DbusState {
    connections: Vec<Connection>,
}

// TODO: Think about if all dbus services should run on the same thread

pub fn run_dbus_services() -> anyhow::Result<()> {
    let mut event_loop = EventLoop::<'static, DbusState>::try_new()?;
    let loop_handle = event_loop.handle();
    let (executor, scheduler) = calloop::futures::executor()?;

    loop_handle
        .insert_source(executor, |event, (), state| {
            info!("Finished futures");
            match event {
                Ok(event) => {
                    state.connections.push(event);
                }
                Err(err) => {
                    error!("Error running futures: {:?}", err);
                }
            }
        })
        .unwrap();

    // let future = org_gnome_mutter_screencast::start();
    // scheduler.schedule(future)?;
    let future = portals::start();
    scheduler.schedule(future)?;

    let mut state = DbusState {
        connections: Vec::new(),
    };
    event_loop.run(None, &mut state, |_| {})?;

    Ok(())
}
