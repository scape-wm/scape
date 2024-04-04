use crate::State;
use _screencopy::zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1;
use _screencopy::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;
use smithay::output::Output;
use smithay::reexports::wayland_protocols_wlr::screencopy::v1::server as _screencopy;
use smithay::reexports::wayland_protocols_wlr::screencopy::v1::server::zwlr_screencopy_frame_v1::Flags;
use smithay::reexports::wayland_protocols_wlr::screencopy::v1::server::zwlr_screencopy_frame_v1::Request as FrameRequest;
use smithay::reexports::wayland_protocols_wlr::screencopy::v1::server::zwlr_screencopy_manager_v1::Request as ManagerRequest;
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_shm;
use smithay::reexports::wayland_server::{Client, DataInit, Dispatch, DisplayHandle};
use smithay::reexports::wayland_server::{GlobalDispatch, New, Resource};
use smithay::utils::{Physical, Rectangle};
use std::time::UNIX_EPOCH;
use tracing::error;

impl ScreencopyHandler for State {
    fn output(&mut self, output: &WlOutput) -> &Output {
        self.outputs.values().find(|o| o.owns(output)).unwrap()
    }

    fn frame(&mut self, frame: Screencopy) {
        match &self.backend_data {
            crate::state::BackendData::None => panic!("Cannot craete screencopy without backend"),
            crate::state::BackendData::Udev(udev_data) => {
                for (node, device) in &udev_data.backends {
                    for (crtc, surface) in &device.surfaces {
                        if surface.output == frame.output {
                            crate::udev::render(self, *node, Some(*crtc), Some(frame));
                            return;
                        }
                    }
                }
            }
            crate::state::BackendData::Winit(_) => {
                error!("Screencopy is not implemented for winit");
                frame.failed();
            }
        }
    }
}

const MANAGER_VERSION: u32 = 3;

pub struct ScreencopyManagerState;

impl ScreencopyManagerState {
    pub fn new<D>(display: &DisplayHandle) -> Self
    where
        D: GlobalDispatch<ZwlrScreencopyManagerV1, ()>,
        D: Dispatch<ZwlrScreencopyManagerV1, ()>,
        D: Dispatch<ZwlrScreencopyFrameV1, ScreencopyFrameState>,
        D: ScreencopyHandler,
        D: 'static,
    {
        display.create_global::<D, ZwlrScreencopyManagerV1, _>(MANAGER_VERSION, ());

        Self
    }
}

impl<D> GlobalDispatch<ZwlrScreencopyManagerV1, (), D> for ScreencopyManagerState
where
    D: GlobalDispatch<ZwlrScreencopyManagerV1, ()>,
    D: Dispatch<ZwlrScreencopyManagerV1, ()>,
    D: Dispatch<ZwlrScreencopyFrameV1, ScreencopyFrameState>,
    D: ScreencopyHandler,
    D: 'static,
{
    fn bind(
        _state: &mut D,
        _display: &DisplayHandle,
        _client: &Client,
        manager: New<ZwlrScreencopyManagerV1>,
        _manager_state: &(),
        data_init: &mut DataInit<'_, D>,
    ) {
        data_init.init(manager, ());
    }
}

impl<D> Dispatch<ZwlrScreencopyManagerV1, (), D> for ScreencopyManagerState
where
    D: GlobalDispatch<ZwlrScreencopyManagerV1, ()>,
    D: Dispatch<ZwlrScreencopyManagerV1, ()>,
    D: Dispatch<ZwlrScreencopyFrameV1, ScreencopyFrameState>,
    D: ScreencopyHandler,
    D: 'static,
{
    fn request(
        state: &mut D,
        _client: &Client,
        manager: &ZwlrScreencopyManagerV1,
        request: ManagerRequest,
        _data: &(),
        _display: &DisplayHandle,
        data_init: &mut DataInit<'_, D>,
    ) {
        let (frame, overlay_cursor, rect, output) = match request {
            ManagerRequest::CaptureOutput {
                frame,
                overlay_cursor,
                output,
            } => {
                let output = state.output(&output);
                let rect =
                    Rectangle::from_loc_and_size((0, 0), output.current_mode().unwrap().size);
                (frame, overlay_cursor, rect, output.clone())
            }
            ManagerRequest::CaptureOutputRegion {
                frame,
                overlay_cursor,
                x,
                y,
                width,
                height,
                output,
            } => {
                let rect = Rectangle::from_loc_and_size((x, y), (width, height));

                // Translate logical rect to physical framebuffer coordinates.
                let output = state.output(&output);
                let output_transform = output.current_transform();
                let rotated_rect =
                    output_transform.transform_rect_in(rect, &output.current_mode().unwrap().size);

                // Clamp captured region to the output.
                let clamped_rect = rotated_rect
                    .intersection(Rectangle::from_loc_and_size(
                        (0, 0),
                        output.current_mode().unwrap().size,
                    ))
                    .unwrap_or_default();

                (frame, overlay_cursor, clamped_rect, output.clone())
            }
            ManagerRequest::Destroy => return,
            _ => unreachable!(),
        };

        // Create the frame.
        let overlay_cursor = overlay_cursor != 0;
        let frame = data_init.init(
            frame,
            ScreencopyFrameState {
                overlay_cursor,
                rect,
                output,
            },
        );

        // Send desired SHM buffer parameters.
        frame.buffer(
            wl_shm::Format::Argb8888,
            rect.size.w as u32,
            rect.size.h as u32,
            rect.size.w as u32 * 4,
        );

        if manager.version() >= 3 {
            // Notify client that all supported buffers were enumerated.
            frame.buffer_done();
        }
    }
}

/// Handler trait for wlr-screencopy.
pub trait ScreencopyHandler {
    /// Get the physical size of an output.
    fn output(&mut self, output: &WlOutput) -> &Output;

    /// Handle new screencopy request.
    fn frame(&mut self, frame: Screencopy);
}

#[allow(missing_docs)]
macro_rules! delegate_screencopy_manager {
    ($(@<$( $lt:tt $( : $clt:tt $(+ $dlt:tt )* )? ),+>)? $ty: ty) => {
        smithay::reexports::wayland_server::delegate_global_dispatch!($(@< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? $ty: [
            smithay::reexports::wayland_protocols_wlr::screencopy::v1::server::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1: ()
        ] => $crate::protocols::wlr_screencopy::ScreencopyManagerState);

        smithay::reexports::wayland_server::delegate_dispatch!($(@< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? $ty: [
            smithay::reexports::wayland_protocols_wlr::screencopy::v1::server::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1: ()
        ] => $crate::protocols::wlr_screencopy::ScreencopyManagerState);

        smithay::reexports::wayland_server::delegate_dispatch!($(@< $( $lt $( : $clt $(+ $dlt )* )? ),+ >)? $ty: [
            smithay::reexports::wayland_protocols_wlr::screencopy::v1::server::zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1: $crate::protocols::wlr_screencopy::ScreencopyFrameState
        ] => $crate::protocols::wlr_screencopy::ScreencopyManagerState);
    };
}

pub struct ScreencopyFrameState {
    pub rect: Rectangle<i32, Physical>,
    pub overlay_cursor: bool,
    pub output: Output,
}

impl<D> Dispatch<ZwlrScreencopyFrameV1, ScreencopyFrameState, D> for ScreencopyManagerState
where
    D: Dispatch<ZwlrScreencopyFrameV1, ScreencopyFrameState>,
    D: ScreencopyHandler,
    D: 'static,
{
    fn request(
        state: &mut D,
        _client: &Client,
        frame: &ZwlrScreencopyFrameV1,
        request: FrameRequest,
        data: &ScreencopyFrameState,
        _display: &DisplayHandle,
        _data_init: &mut DataInit<'_, D>,
    ) {
        let (buffer, send_damage) = match request {
            FrameRequest::Copy { buffer } => (buffer, false),
            FrameRequest::CopyWithDamage { buffer } => (buffer, true),
            FrameRequest::Destroy => return,
            _ => unreachable!(),
        };

        state.frame(Screencopy {
            send_damage,
            buffer,
            frame: frame.clone(),
            region: data.rect,
            submitted: false,
            output: data.output.clone(),
            overlay_cursor: data.overlay_cursor,
        });
    }
}

/// Screencopy frame.
pub struct Screencopy {
    region: Rectangle<i32, Physical>,
    frame: ZwlrScreencopyFrameV1,
    send_damage: bool,
    buffer: WlBuffer,
    submitted: bool,
    pub output: Output,
    pub overlay_cursor: bool,
}

impl Drop for Screencopy {
    fn drop(&mut self) {
        if !self.submitted {
            self.frame.failed();
        }
    }
}

impl Screencopy {
    /// Get the target buffer to copy to.
    pub fn buffer(&self) -> &WlBuffer {
        &self.buffer
    }

    /// Get the region which should be copied.
    pub fn region(&self) -> Rectangle<i32, Physical> {
        self.region
    }

    /// Mark damaged regions of the screencopy buffer.
    pub fn damage(&mut self, damage: &[Rectangle<i32, Physical>]) {
        if !self.send_damage {
            return;
        }

        for Rectangle { loc, size } in damage {
            self.frame
                .damage(loc.x as u32, loc.y as u32, size.w as u32, size.h as u32);
        }
    }

    /// Submit the copied content.
    pub fn submit(mut self) {
        // Notify client that buffer is ordinary.
        self.frame.flags(Flags::empty());

        // Notify client about successful copy.
        let now = UNIX_EPOCH.elapsed().unwrap();
        let secs = now.as_secs();
        self.frame
            .ready((secs >> 32) as u32, secs as u32, now.subsec_nanos());

        // Mark frame as submitted to ensure destructor isn't run.
        self.submitted = true;
    }

    pub fn failed(self) {
        self.frame.failed();
    }
}

delegate_screencopy_manager!(State);
