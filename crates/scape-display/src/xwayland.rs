use crate::State;
use anyhow::anyhow;
use smithay::{
    utils::{Point, Size},
    wayland::xwayland_keyboard_grab::XWaylandKeyboardGrabState,
    xwayland::{X11Wm, XWayland, XWaylandEvent},
};
use std::process::Stdio;
use tracing::{error, info};

#[derive(Debug)]
pub struct XWaylandState {
    pub wm: Option<X11Wm>,
    pub display_number: Option<u32>,
    pub grab_state: XWaylandKeyboardGrabState,
}

impl State {
    pub fn start_xwayland(&mut self) -> anyhow::Result<()> {
        let (xwayland, client) = XWayland::spawn(
            &self.display_handle,
            None,
            std::iter::empty::<(String, String)>(),
            true,
            Stdio::null(),
            Stdio::null(),
            |_| (),
        )?;
        self.loop_handle
            .insert_source(xwayland, move |event, _, state| match event {
                XWaylandEvent::Ready {
                    x11_socket,
                    display_number,
                } => {
                    let Ok(mut wm) =
                        X11Wm::start_wm(state.loop_handle.clone(), x11_socket, client.clone())
                            .inspect_err(
                                |e| error!(err = %e,"Failed to attach X11 Window Manager"),
                            )
                    else {
                        return;
                    };

                    let image = state.cursor_state.get_default_image();
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
                        xwayland_state.display_number = Some(display_number);
                    } else {
                        error!("Unable to set xwayland wm/display, since the state is missing");
                    }

                    ::std::env::set_var("DISPLAY", format!("{display_number}"));
                    info!("XWayland started");

                    state.xwayland_ready();
                }
                XWaylandEvent::Error => {
                    info!("XWayland could not be started");
                    state.xwayland_state = None;
                }
            })
            .map_err(|e| {
                anyhow!(
                    "Failed to insert the XWaylandSource into the event loop: {}",
                    e
                )
            })?;

        let grab_state = XWaylandKeyboardGrabState::new::<Self>(&self.display_handle);

        self.xwayland_state = Some(XWaylandState {
            display_number: None,
            wm: None,
            grab_state,
        });

        Ok(())
    }
}
