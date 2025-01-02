//! The renderer module is responsible for rendering various UI elements based on the current state.

#![warn(missing_docs)]

mod drm;
mod gbm;
mod udev;
mod vulkan;

use std::{
    collections::{HashMap, HashSet},
    os::fd::{AsFd, BorrowedFd, OwnedFd},
};

use ::drm::node::DrmNode;
use anyhow::Context;
use calloop::{LoopHandle, LoopSignal};
use scape_shared::{Comms, GlobalArgs, MainMessage, MessageRunner, RendererMessage};
use tracing::info;

struct Gpu {
    node: DrmNode,
    fd: OwnedFd,
}

impl AsFd for Gpu {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

/// Holds the state of the renderer module
pub struct RendererState {
    comms: Comms,
    shutting_down: bool,
    loop_handle: LoopHandle<'static, RendererState>,
    primary_gpu: Option<DrmNode>,
    known_drm_devices: HashSet<DrmNode>,
    gpus: HashMap<DrmNode, Gpu>,
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
            primary_gpu: None,
            known_drm_devices: HashSet::new(),
            gpus: HashMap::new(),
        })
    }

    fn handle_message(&mut self, message: RendererMessage) -> anyhow::Result<()> {
        match message {
            RendererMessage::Shutdown => {
                self.shutting_down = true;
            }
            RendererMessage::SeatSessionCreated { seat_name } => {
                self.init_udev_device_listener_for_seat(&seat_name)
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
            RendererMessage::FileOpenedInSession { path, fd } => {
                let Some(primary_gpu) = self.primary_gpu else {
                    self.comms.main(MainMessage::Shutdown);
                    anyhow::bail!("No primary gpu available");
                };
                let node = DrmNode::from_path(path)?;
                let gpu = Gpu { node, fd };
                // self.gpus.insert(node, gpu);
                // self.test_drm();

                let (executor, scheduler) = calloop::futures::executor::<anyhow::Result<()>>()?;
                self.loop_handle
                    .insert_source(executor, |event, (), state| {
                        info!("Finished futures {:?}", event);
                    })
                    .unwrap();
                let future = drm::test_wgpu(gpu);
                scheduler.schedule(future)?;
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
