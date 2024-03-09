use crate::{cursor::Cursor, State};
use anyhow::anyhow;
use smithay::{
    utils::{Point, Size},
    wayland::xwayland_keyboard_grab::XWaylandKeyboardGrabState,
    xwayland::{X11Wm, XWayland, XWaylandEvent},
};
use std::{ffi::OsString, time::Duration};
use tracing::{error, info};

#[derive(Debug)]
pub struct XWaylandState {
    pub xwayland: XWayland,
    pub wm: Option<X11Wm>,
    pub display: Option<u32>,
    pub grab_state: XWaylandKeyboardGrabState,
}

impl State {
    pub fn start_xwayland(&mut self) -> anyhow::Result<()> {
        let (xwayland, channel) = XWayland::new(&self.display_handle);
        self.loop_handle
            .insert_source(channel, move |event, _, state| match event {
                XWaylandEvent::Ready {
                    connection,
                    client,
                    client_fd: _,
                    display,
                } => {
                    let Ok(mut wm) = X11Wm::start_wm(
                        state.loop_handle.clone(),
                        state.display_handle.clone(),
                        connection,
                        client,
                    )
                    .inspect_err(|e| error!(err = %e,"Failed to attach X11 Window Manager")) else {
                        return;
                    };

                    let cursor = Cursor::load();
                    let image = cursor.get_image(1, Duration::ZERO);
                    if let Err(e) = wm.set_cursor(
                        &image.pixels_rgba,
                        Size::from((image.width as u16, image.height as u16)),
                        Point::from((image.xhot as u16, image.yhot as u16)),
                    ) {
                        error!(err = %e, "Failed to set xwayland default cursor");
                        return;
                    }

                    if let Some(xwayland_state) = &mut state.xwayland_state {
                        xwayland_state.wm = Some(wm);
                        xwayland_state.display = Some(display);
                    } else {
                        error!("Unable to set xwayland wm/display, since the state is missing");
                    }

                    info!("XWayland started");
                }
                XWaylandEvent::Exited => {
                    info!("XWayland exited");
                    state.xwayland_state = None;
                }
            })
            .map_err(|e| {
                anyhow!(
                    "Failed to insert the XWaylandSource into the event loop: {}",
                    e
                )
            })?;

        xwayland
            .start(
                self.loop_handle.clone(),
                None,
                std::iter::empty::<(OsString, OsString)>(), // TODO: Add configuration option
                true,
                |_| {},
            )
            .map_err(|e| anyhow!("Failed to start xwayland: {}", e))?;

        let grab_state = XWaylandKeyboardGrabState::new::<Self>(&self.display_handle);

        self.xwayland_state = Some(XWaylandState {
            xwayland,
            display: None,
            wm: None,
            grab_state,
        });

        Ok(())
    }
}
