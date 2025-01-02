use std::os::fd::AsRawFd;

use crate::RendererState;
use drm::control::{self, crtc, framebuffer, Device as _};
use gbm::{BufferObjectFlags, Device as GbmDevice, Format};

impl RendererState {
    pub(crate) fn test_gbm(&mut self) {
        let gpu = &self.gpus.values().next().unwrap();

        let gbm = GbmDevice::new(gpu).unwrap();

        let mut bo = gbm
            .create_buffer_object::<()>(
                1280,
                720,
                Format::Argb8888,
                BufferObjectFlags::SCANOUT | BufferObjectFlags::WRITE,
            )
            .unwrap();
        let surface = gbm
            .create_surface::<()>(1280, 720, Format::Argb8888, BufferObjectFlags::SCANOUT)
            .unwrap();

        // surface.lock_front_buffer()

        let fb = gpu.add_framebuffer(&bo, 0, 0).unwrap();
    }
}
