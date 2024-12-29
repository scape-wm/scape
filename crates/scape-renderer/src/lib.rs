//! The renderer module is responsible for rendering various UI elements based on the current state.

#![warn(missing_docs)]

mod drm;
mod udev;

use anyhow::Context;
use calloop::{LoopHandle, LoopSignal};
use scape_shared::{Comms, GlobalArgs, MessageRunner, RendererMessage};
use udev::init_udev_device_listener_for_seat;

/// Holds the state of the renderer module
pub struct RendererState {
    comms: Comms,
    shutting_down: bool,
    loop_handle: LoopHandle<'static, RendererState>,
}

impl MessageRunner for RendererState {
    type Message = RendererMessage;

    fn new(
        comms: Comms,
        loop_handle: LoopHandle<'static, RendererState>,
        _args: &GlobalArgs,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            comms,
            shutting_down: false,
            loop_handle,
        })
    }

    fn handle_message(&mut self, message: RendererMessage) -> anyhow::Result<()> {
        match message {
            RendererMessage::Shutdown => {
                self.shutting_down = true;
            }
            RendererMessage::SeatSessionCreated { seat_name } => {
                init_udev_device_listener_for_seat(seat_name, self.loop_handle.clone())
                    .context("Unable to init udev device listener")?;
            }
            RendererMessage::SeatSessionPaused => {
                // TODO: Properly handle seat session pauses
                // state.session_paused = true;
                // for backend in udev_data.backends.values_mut() {
                //     backend.drm.pause();
                //     backend.active_leases.clear();
                //     if let Some(lease_global) = backend.leasing_global.as_mut() {
                //         lease_global.suspend();
                //     }
                // }
            }
            RendererMessage::SeatSessionResumed => {
                // TODO: Properly handle seat session resumes
                // state.session_paused = false;
                // for (_node, backend) in udev_data
                //     .backends
                //     .iter_mut()
                //     .map(|(handle, backend)| (*handle, backend))
                // {
                //     if let Err(err) = backend.drm.activate(false) {
                //         warn!(?err, "Unable to actiave drm");
                //     }
                //     if let Some(lease_global) = backend.leasing_global.as_mut() {
                //         lease_global.resume::<State>();
                //     }
                //     for surface in backend.surfaces.values_mut() {
                //         if let Err(err) = surface.compositor.reset_state() {
                //             warn!("Failed to reset drm surface state: {}", err);
                //         }
                //         // reset the buffers after resume to trigger a full redraw
                //         // this is important after a vt switch as the primary plane
                //         // has no content and damage tracking may prevent a redraw
                //         // otherwise
                //         surface.compositor.reset_buffers();
                //     }
                // }
                //
                // state.backend_data.schedule_render();
            }
        }

        Ok(())
    }

    fn on_dispatch_wait(&mut self, signal: &LoopSignal) {
        if self.shutting_down {
            signal.stop();
        }
    }
}
