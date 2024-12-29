use calloop::LoopHandle;
use smithay::{
    backend::{
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        session::{libseat::LibSeatSession, Session},
    },
    reexports::input::Libinput,
};

use crate::InputState;

pub(crate) fn start_input(
    loop_handle: LoopHandle<'static, InputState>,
    session: LibSeatSession,
) -> anyhow::Result<Libinput> {
    let seat_name = session.seat();
    let mut libinput_context =
        Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(session.into());
    if libinput_context.udev_assign_seat(&seat_name).is_err() {
        anyhow::bail!("Failed to assign seat to libinput context");
    }

    let libinput_backend = LibinputInputBackend::new(libinput_context.clone());
    loop_handle
        .insert_source(libinput_backend, move |event, _, state| {
            state.handle_input_event(event)
        })
        .map_err(|e| anyhow::anyhow!("Unable to insert libinput source: {}", e))?;

    Ok(libinput_context)
}
