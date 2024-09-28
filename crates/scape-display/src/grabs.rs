use crate::workspace_window::WorkspaceWindow;
use crate::{focus::PointerFocusTarget, state::State};
use smithay::input::touch::{
    GrabStartData as TouchGrabStartData, OrientationEvent, ShapeEvent, TouchGrab,
};
use smithay::xwayland::xwm::ResizeEdge as X11ResizeEdge;
use smithay::{
    input::pointer::{
        AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent,
        GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent,
        GestureSwipeEndEvent, GestureSwipeUpdateEvent, GrabStartData as PointerGrabStartData,
        MotionEvent, PointerGrab, PointerInnerHandle, RelativeMotionEvent,
    },
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel,
    utils::{IsAlive, Logical, Point, Serial, Size},
    wayland::{compositor::with_states, shell::xdg::SurfaceCachedState},
};
use tracing::warn;

pub struct PointerMoveSurfaceGrab {
    pub start_data: PointerGrabStartData<State>,
    pub window: WorkspaceWindow,
    pub space_name: String,
    pub initial_window_location: Point<i32, Logical>,
}

impl PointerGrab<State> for PointerMoveSurfaceGrab {
    fn motion(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        _focus: Option<(PointerFocusTarget, Point<i32, Logical>)>,
        event: &MotionEvent,
    ) {
        // While the grab is active, no client has pointer focus
        handle.motion(data, None, event);

        let delta = event.location - self.start_data.location;
        let new_location = self.initial_window_location.to_f64() + delta;

        data.spaces.get_mut(&self.space_name).unwrap().map_element(
            self.window.clone(),
            new_location.to_i32_round(),
            true,
        );
    }

    fn relative_motion(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        focus: Option<(PointerFocusTarget, Point<i32, Logical>)>,
        event: &RelativeMotionEvent,
    ) {
        handle.relative_motion(data, focus, event);
    }

    fn button(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &ButtonEvent,
    ) {
        handle.button(data, event);
        if handle.current_pressed().is_empty() {
            // No more buttons are pressed, release the grab.
            handle.unset_grab(self, data, event.serial, event.time, true);
        }
    }

    fn axis(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        details: AxisFrame,
    ) {
        handle.axis(data, details)
    }

    fn start_data(&self) -> &PointerGrabStartData<State> {
        &self.start_data
    }

    fn frame(&mut self, data: &mut State, handle: &mut PointerInnerHandle<'_, State>) {
        handle.frame(data);
    }

    fn gesture_swipe_begin(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GestureSwipeBeginEvent,
    ) {
        handle.gesture_swipe_begin(data, event);
    }

    fn gesture_swipe_update(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GestureSwipeUpdateEvent,
    ) {
        handle.gesture_swipe_update(data, event);
    }

    fn gesture_swipe_end(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GestureSwipeEndEvent,
    ) {
        handle.gesture_swipe_end(data, event);
    }

    fn gesture_pinch_begin(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GesturePinchBeginEvent,
    ) {
        handle.gesture_pinch_begin(data, event);
    }

    fn gesture_pinch_update(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GesturePinchUpdateEvent,
    ) {
        handle.gesture_pinch_update(data, event);
    }

    fn gesture_pinch_end(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GesturePinchEndEvent,
    ) {
        handle.gesture_pinch_end(data, event);
    }

    fn gesture_hold_begin(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GestureHoldBeginEvent,
    ) {
        handle.gesture_hold_begin(data, event);
    }

    fn gesture_hold_end(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GestureHoldEndEvent,
    ) {
        handle.gesture_hold_end(data, event);
    }

    fn unset(&mut self, _data: &mut State) {}
}

pub struct TouchMoveSurfaceGrab {
    pub start_data: TouchGrabStartData<State>,
    pub window: WorkspaceWindow,
    pub initial_window_location: Point<i32, Logical>,
}

impl TouchGrab<State> for TouchMoveSurfaceGrab {
    fn down(
        &mut self,
        _data: &mut State,
        _handle: &mut smithay::input::touch::TouchInnerHandle<'_, State>,
        _focus: Option<(
            <State as smithay::input::SeatHandler>::TouchFocus,
            Point<i32, Logical>,
        )>,
        _event: &smithay::input::touch::DownEvent,
        _seq: Serial,
    ) {
    }

    fn up(
        &mut self,
        data: &mut State,
        handle: &mut smithay::input::touch::TouchInnerHandle<'_, State>,
        event: &smithay::input::touch::UpEvent,
        seq: Serial,
    ) {
        if event.slot != self.start_data.slot {
            return;
        }

        handle.up(data, event, seq);
        handle.unset_grab(self, data);
    }

    fn motion(
        &mut self,
        data: &mut State,
        _handle: &mut smithay::input::touch::TouchInnerHandle<'_, State>,
        _focus: Option<(
            <State as smithay::input::SeatHandler>::TouchFocus,
            Point<i32, Logical>,
        )>,
        event: &smithay::input::touch::MotionEvent,
        _seq: Serial,
    ) {
        if event.slot != self.start_data.slot {
            return;
        }

        let delta = event.location - self.start_data.location;
        let new_location = self.initial_window_location.to_f64() + delta;
        // TODO: find out from which space the window is
        data.spaces.values_mut().next().unwrap().map_element(
            self.window.clone(),
            new_location.to_i32_round(),
            true,
        );
    }

    fn frame(
        &mut self,
        _data: &mut State,
        _handle: &mut smithay::input::touch::TouchInnerHandle<'_, State>,
        _seq: Serial,
    ) {
    }

    fn cancel(
        &mut self,
        data: &mut State,
        handle: &mut smithay::input::touch::TouchInnerHandle<'_, State>,
        seq: Serial,
    ) {
        handle.cancel(data, seq);
        handle.unset_grab(self, data);
    }

    fn start_data(&self) -> &smithay::input::touch::GrabStartData<State> {
        &self.start_data
    }

    fn shape(
        &mut self,
        data: &mut State,
        handle: &mut smithay::input::touch::TouchInnerHandle<'_, State>,
        event: &ShapeEvent,
        seq: Serial,
    ) {
        handle.shape(data, event, seq);
    }

    fn orientation(
        &mut self,
        data: &mut State,
        handle: &mut smithay::input::touch::TouchInnerHandle<'_, State>,
        event: &OrientationEvent,
        seq: Serial,
    ) {
        handle.orientation(data, event, seq);
    }

    fn unset(&mut self, _data: &mut State) {}
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, Eq, PartialEq)]
    pub struct ResizeEdge: u32 {
        const NONE = 0;
        const TOP = 1;
        const BOTTOM = 2;
        const LEFT = 4;
        const TOP_LEFT = 5;
        const BOTTOM_LEFT = 6;
        const RIGHT = 8;
        const TOP_RIGHT = 9;
        const BOTTOM_RIGHT = 10;
    }
}

impl From<xdg_toplevel::ResizeEdge> for ResizeEdge {
    #[inline]
    fn from(value: xdg_toplevel::ResizeEdge) -> Self {
        match value {
            xdg_toplevel::ResizeEdge::None => ResizeEdge::NONE,
            xdg_toplevel::ResizeEdge::Top => ResizeEdge::TOP,
            xdg_toplevel::ResizeEdge::Bottom => ResizeEdge::BOTTOM,
            xdg_toplevel::ResizeEdge::Left => ResizeEdge::LEFT,
            xdg_toplevel::ResizeEdge::TopLeft => ResizeEdge::TOP_LEFT,
            xdg_toplevel::ResizeEdge::BottomLeft => ResizeEdge::BOTTOM_LEFT,
            xdg_toplevel::ResizeEdge::Right => ResizeEdge::RIGHT,
            xdg_toplevel::ResizeEdge::TopRight => ResizeEdge::TOP_RIGHT,
            xdg_toplevel::ResizeEdge::BottomRight => ResizeEdge::BOTTOM_RIGHT,
            _ => {
                warn!("xdg_toplevel::ResizeEdge value of {value:?} cannot be mapped");
                ResizeEdge::NONE
            }
        }
    }
}

impl From<ResizeEdge> for xdg_toplevel::ResizeEdge {
    #[inline]
    fn from(value: ResizeEdge) -> Self {
        match value {
            ResizeEdge::NONE => xdg_toplevel::ResizeEdge::None,
            ResizeEdge::TOP => xdg_toplevel::ResizeEdge::Top,
            ResizeEdge::BOTTOM => xdg_toplevel::ResizeEdge::Bottom,
            ResizeEdge::LEFT => xdg_toplevel::ResizeEdge::Left,
            ResizeEdge::TOP_LEFT => xdg_toplevel::ResizeEdge::TopLeft,
            ResizeEdge::BOTTOM_LEFT => xdg_toplevel::ResizeEdge::BottomLeft,
            ResizeEdge::RIGHT => xdg_toplevel::ResizeEdge::Right,
            ResizeEdge::TOP_RIGHT => xdg_toplevel::ResizeEdge::TopRight,
            ResizeEdge::BOTTOM_RIGHT => xdg_toplevel::ResizeEdge::BottomRight,
            _ => {
                warn!("ResizeEdge value of {value:?} cannot be mapped");
                xdg_toplevel::ResizeEdge::None
            }
        }
    }
}

impl From<X11ResizeEdge> for ResizeEdge {
    fn from(edge: X11ResizeEdge) -> Self {
        match edge {
            X11ResizeEdge::Bottom => ResizeEdge::BOTTOM,
            X11ResizeEdge::BottomLeft => ResizeEdge::BOTTOM_LEFT,
            X11ResizeEdge::BottomRight => ResizeEdge::BOTTOM_RIGHT,
            X11ResizeEdge::Left => ResizeEdge::LEFT,
            X11ResizeEdge::Right => ResizeEdge::RIGHT,
            X11ResizeEdge::Top => ResizeEdge::TOP,
            X11ResizeEdge::TopLeft => ResizeEdge::TOP_LEFT,
            X11ResizeEdge::TopRight => ResizeEdge::TOP_RIGHT,
        }
    }
}

/// Information about the resize operation.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ResizeData {
    /// The edges the surface is being resized with.
    pub edges: ResizeEdge,
    /// The initial window location.
    pub initial_window_location: Point<i32, Logical>,
    /// The initial window size (geometry width and height).
    pub initial_window_size: Size<i32, Logical>,
}

/// State of the resize operation.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Default)]
pub enum ResizeState {
    /// The surface is not being resized.
    #[default]
    NotResizing,
    /// The surface is currently being resized.
    Resizing(ResizeData),
    /// The resize has finished, and the surface needs to ack the final configure.
    WaitingForFinalAck(ResizeData, Serial),
    /// The resize has finished, and the surface needs to commit its final state.
    WaitingForCommit(ResizeData),
}

pub struct PointerResizeSurfaceGrab {
    pub start_data: PointerGrabStartData<State>,
    pub window: WorkspaceWindow,
    pub space_name: String,
    pub edges: ResizeEdge,
    pub initial_window_location: Point<i32, Logical>,
    pub initial_window_size: Size<i32, Logical>,
    pub last_window_size: Size<i32, Logical>,
}

impl PointerGrab<State> for PointerResizeSurfaceGrab {
    fn motion(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        _focus: Option<(PointerFocusTarget, Point<i32, Logical>)>,
        event: &MotionEvent,
    ) {
        // While the grab is active, no client has pointer focus
        handle.motion(data, None, event);

        // It is impossible to get `min_size` and `max_size` of dead toplevel, so we return early.
        if !self.window.alive() {
            handle.unset_grab(self, data, event.serial, event.time, true);
            return;
        }

        let (mut dx, mut dy) = (event.location - self.start_data.location).into();

        let mut new_window_width = self.initial_window_size.w;
        let mut new_window_height = self.initial_window_size.h;

        let left_right = ResizeEdge::LEFT | ResizeEdge::RIGHT;
        let top_bottom = ResizeEdge::TOP | ResizeEdge::BOTTOM;

        if self.edges.intersects(left_right) {
            if self.edges.intersects(ResizeEdge::LEFT) {
                dx = -dx;
            }

            new_window_width = (self.initial_window_size.w as f64 + dx) as i32;
        }

        if self.edges.intersects(top_bottom) {
            if self.edges.intersects(ResizeEdge::TOP) {
                dy = -dy;
            }

            new_window_height = (self.initial_window_size.h as f64 + dy) as i32;
        }

        let (min_size, max_size) = if let Some(surface) = self.window.wl_surface() {
            with_states(&surface, |states| {
                let data = states.cached_state.current::<SurfaceCachedState>();
                (data.min_size, data.max_size)
            })
        } else {
            ((0, 0).into(), (0, 0).into())
        };

        let min_width = min_size.w.max(1);
        let min_height = min_size.h.max(1);
        let max_width = if max_size.w == 0 {
            i32::max_value()
        } else {
            max_size.w
        };
        let max_height = if max_size.h == 0 {
            i32::max_value()
        } else {
            max_size.h
        };

        new_window_width = new_window_width.clamp(min_width, max_width);
        new_window_height = new_window_height.clamp(min_height, max_height);

        self.last_window_size = (new_window_width, new_window_height).into();

        if let Some(space_name) = data.space_of_window(&self.window) {
            if let Some(location) = data.spaces[&space_name].element_location(&self.window) {
                self.window.resize(location, self.last_window_size);
            }
        }
    }

    fn relative_motion(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        focus: Option<(PointerFocusTarget, Point<i32, Logical>)>,
        event: &RelativeMotionEvent,
    ) {
        handle.relative_motion(data, focus, event);
    }

    fn button(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &ButtonEvent,
    ) {
        handle.button(data, event);
        if handle.current_pressed().is_empty() {
            // No more buttons are pressed, release the grab.
            handle.unset_grab(self, data, event.serial, event.time, true);

            // If toplevel is dead, we can't resize it, so we return early.
            // if !self.window.alive() {
            //     return;
            // }

            // TODO: Impl resizing using cursor
            // match &self.window.0.underlying_surface() {
            //     WindowSurface::Wayland(xdg) => {
            //         xdg.with_pending_state(|state| {
            //             state.states.unset(xdg_toplevel::State::Resizing);
            //             state.size = Some(self.last_window_size);
            //         });
            //         xdg.send_pending_configure();
            //         if self.edges.intersects(ResizeEdge::TOP_LEFT) {
            //             let geometry = self.window.geometry();
            //             let Some(mut location) =
            //                 data.spaces[&self.space_name].element_location(&self.window)
            //             else {
            //                 warn!("Window not found in space: {:?}", self.window);
            //                 return;
            //             };
            //
            //             if self.edges.intersects(ResizeEdge::LEFT) {
            //                 location.x = self.initial_window_location.x
            //                     + (self.initial_window_size.w - geometry.size.w);
            //             }
            //             if self.edges.intersects(ResizeEdge::TOP) {
            //                 location.y = self.initial_window_location.y
            //                     + (self.initial_window_size.h - geometry.size.h);
            //             }
            //
            //             data.spaces.get_mut(&self.space_name).unwrap().map_element(
            //                 self.window.clone(),
            //                 location,
            //                 true,
            //             );
            //         }
            //
            //         let Some(wl_surface) = &self.window.wl_surface() else {
            //             warn!("Wl_surface not found on window {:?}", self.window);
            //             return;
            //         };
            //         with_states(wl_surface, |states| {
            //             let Some(surface_data) = states.data_map.get::<RefCell<SurfaceData>>()
            //             else {
            //                 warn!("No surface data found on surface {:?}", wl_surface);
            //                 return;
            //             };
            //             let Ok(mut data) = surface_data.try_borrow_mut() else {
            //                 warn!(
            //                     "Cannot borrow surface data as mut of surface {:?}",
            //                     wl_surface
            //                 );
            //                 return;
            //             };
            //             if let ResizeState::Resizing(resize_data) = data.resize_state {
            //                 data.resize_state =
            //                     ResizeState::WaitingForFinalAck(resize_data, event.serial);
            //             } else {
            //                 warn!("Invalid resize state: {:?}", data.resize_state);
            //             }
            //         });
            //     }
            //     WindowSurface::X11(x11) => {
            //         let Some(mut location) =
            //             data.spaces[&self.space_name].element_location(&self.window)
            //         else {
            //             warn!("Window {:?} not found in space", self.window);
            //             return;
            //         };
            //         if self.edges.intersects(ResizeEdge::TOP_LEFT) {
            //             let geometry = self.window.geometry();
            //
            //             if self.edges.intersects(ResizeEdge::LEFT) {
            //                 location.x = self.initial_window_location.x
            //                     + (self.initial_window_size.w - geometry.size.w);
            //             }
            //             if self.edges.intersects(ResizeEdge::TOP) {
            //                 location.y = self.initial_window_location.y
            //                     + (self.initial_window_size.h - geometry.size.h);
            //             }
            //
            //             data.spaces.get_mut(&self.space_name).unwrap().map_element(
            //                 self.window.clone(),
            //                 location,
            //                 true,
            //             );
            //         }
            //         if let Err(e) = x11.configure(Rectangle::from_loc_and_size(
            //             location,
            //             self.last_window_size,
            //         )) {
            //             error!(
            //                 "Unable to configure new location on X11 surface {:?}: {}",
            //                 x11, e
            //             );
            //         }
            //
            //         let Some(surface) = self.window.wl_surface() else {
            //             // X11 Window got unmapped, abort
            //             return;
            //         };
            //         with_states(&surface, |states| {
            //             let Some(surface_data) = states.data_map.get::<RefCell<SurfaceData>>()
            //             else {
            //                 warn!("No surface data found on surface {:?}", surface);
            //                 return;
            //             };
            //             let Ok(mut data) = surface_data.try_borrow_mut() else {
            //                 warn!("Cannot borrow surface data as mut of surface {:?}", surface);
            //                 return;
            //             };
            //             if let ResizeState::Resizing(resize_data) = data.resize_state {
            //                 data.resize_state = ResizeState::WaitingForCommit(resize_data);
            //             } else {
            //                 warn!("Invalid resize state: {:?}", data.resize_state);
            //             }
            //         });
            //     }
            // }
        }
    }

    fn axis(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        details: AxisFrame,
    ) {
        handle.axis(data, details)
    }

    fn start_data(&self) -> &PointerGrabStartData<State> {
        &self.start_data
    }

    fn frame(&mut self, data: &mut State, handle: &mut PointerInnerHandle<'_, State>) {
        handle.frame(data);
    }

    fn gesture_swipe_begin(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GestureSwipeBeginEvent,
    ) {
        handle.gesture_swipe_begin(data, event);
    }

    fn gesture_swipe_update(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GestureSwipeUpdateEvent,
    ) {
        handle.gesture_swipe_update(data, event);
    }

    fn gesture_swipe_end(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GestureSwipeEndEvent,
    ) {
        handle.gesture_swipe_end(data, event);
    }

    fn gesture_pinch_begin(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GesturePinchBeginEvent,
    ) {
        handle.gesture_pinch_begin(data, event);
    }

    fn gesture_pinch_update(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GesturePinchUpdateEvent,
    ) {
        handle.gesture_pinch_update(data, event);
    }

    fn gesture_pinch_end(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GesturePinchEndEvent,
    ) {
        handle.gesture_pinch_end(data, event);
    }

    fn gesture_hold_begin(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GestureHoldBeginEvent,
    ) {
        handle.gesture_hold_begin(data, event);
    }

    fn gesture_hold_end(
        &mut self,
        data: &mut State,
        handle: &mut PointerInnerHandle<'_, State>,
        event: &GestureHoldEndEvent,
    ) {
        handle.gesture_hold_end(data, event);
    }

    fn unset(&mut self, _data: &mut State) {}
}

pub struct TouchResizeSurfaceGrab {
    pub start_data: TouchGrabStartData<State>,
    pub window: WorkspaceWindow,
    pub edges: ResizeEdge,
    pub initial_window_location: Point<i32, Logical>,
    pub initial_window_size: Size<i32, Logical>,
    pub last_window_size: Size<i32, Logical>,
}

impl TouchGrab<State> for TouchResizeSurfaceGrab {
    fn down(
        &mut self,
        _data: &mut State,
        _handle: &mut smithay::input::touch::TouchInnerHandle<'_, State>,
        _focus: Option<(
            <State as smithay::input::SeatHandler>::TouchFocus,
            Point<i32, Logical>,
        )>,
        _event: &smithay::input::touch::DownEvent,
        _seq: Serial,
    ) {
    }

    fn up(
        &mut self,
        data: &mut State,
        handle: &mut smithay::input::touch::TouchInnerHandle<'_, State>,
        event: &smithay::input::touch::UpEvent,
        _seq: Serial,
    ) {
        if event.slot != self.start_data.slot {
            return;
        }
        handle.unset_grab(self, data);

        // If toplevel is dead, we can't resize it, so we return early.
        // if !self.window.alive() {
        //     return;
        // }

        // TODO: Impl resize using cursor
        // match self.window.0.underlying_surface() {
        //     WindowSurface::Wayland(xdg) => {
        //         xdg.with_pending_state(|state| {
        //             state.states.unset(xdg_toplevel::State::Resizing);
        //             state.size = Some(self.last_window_size);
        //         });
        //         xdg.send_pending_configure();
        //         if self.edges.intersects(ResizeEdge::TOP_LEFT) {
        //             let geometry = self.window.geometry();
        //             // TODO: find out from which space this window is
        //             let mut location = data
        //                 .spaces
        //                 .values_mut()
        //                 .next()
        //                 .unwrap()
        //                 .element_location(&self.window)
        //                 .unwrap();
        //
        //             if self.edges.intersects(ResizeEdge::LEFT) {
        //                 location.x = self.initial_window_location.x
        //                     + (self.initial_window_size.w - geometry.size.w);
        //             }
        //             if self.edges.intersects(ResizeEdge::TOP) {
        //                 location.y = self.initial_window_location.y
        //                     + (self.initial_window_size.h - geometry.size.h);
        //             }
        //
        //             // TODO: find out from which space this window is
        //             data.spaces.values_mut().next().unwrap().map_element(
        //                 self.window.clone(),
        //                 location,
        //                 true,
        //             );
        //         }
        //
        //         with_states(&self.window.wl_surface().unwrap(), |states| {
        //             let mut data = states
        //                 .data_map
        //                 .get::<RefCell<SurfaceData>>()
        //                 .unwrap()
        //                 .borrow_mut();
        //             if let ResizeState::Resizing(resize_data) = data.resize_state {
        //                 data.resize_state =
        //                     ResizeState::WaitingForFinalAck(resize_data, event.serial);
        //             } else {
        //                 panic!("invalid resize state: {:?}", data.resize_state);
        //             }
        //         });
        //     }
        //     WindowSurface::X11(x11) => {
        //         // TODO: find out from which space this window is
        //         let mut location = data
        //             .spaces
        //             .values_mut()
        //             .next()
        //             .unwrap()
        //             .element_location(&self.window)
        //             .unwrap();
        //         if self.edges.intersects(ResizeEdge::TOP_LEFT) {
        //             let geometry = self.window.geometry();
        //
        //             if self.edges.intersects(ResizeEdge::LEFT) {
        //                 location.x = self.initial_window_location.x
        //                     + (self.initial_window_size.w - geometry.size.w);
        //             }
        //             if self.edges.intersects(ResizeEdge::TOP) {
        //                 location.y = self.initial_window_location.y
        //                     + (self.initial_window_size.h - geometry.size.h);
        //             }
        //
        //             // TODO: find out from which space this window is
        //             data.spaces.values_mut().next().unwrap().map_element(
        //                 self.window.clone(),
        //                 location,
        //                 true,
        //             );
        //         }
        //         x11.configure(Rectangle::from_loc_and_size(
        //             location,
        //             self.last_window_size,
        //         ))
        //         .unwrap();
        //
        //         let Some(surface) = self.window.wl_surface() else {
        //             // X11 Window got unmapped, abort
        //             return;
        //         };
        //         with_states(&surface, |states| {
        //             let mut data = states
        //                 .data_map
        //                 .get::<RefCell<SurfaceData>>()
        //                 .unwrap()
        //                 .borrow_mut();
        //             if let ResizeState::Resizing(resize_data) = data.resize_state {
        //                 data.resize_state = ResizeState::WaitingForCommit(resize_data);
        //             } else {
        //                 panic!("invalid resize state: {:?}", data.resize_state);
        //             }
        //         });
        //     }
        // }
    }

    fn motion(
        &mut self,
        _data: &mut State,
        _handle: &mut smithay::input::touch::TouchInnerHandle<'_, State>,
        _focus: Option<(
            <State as smithay::input::SeatHandler>::TouchFocus,
            Point<i32, Logical>,
        )>,
        _event: &smithay::input::touch::MotionEvent,
        _seq: Serial,
    ) {
        // if event.slot != self.start_data.slot {
        //     return;
        // }

        // It is impossible to get `min_size` and `max_size` of dead toplevel, so we return early.
        // if !self.window.alive() {
        //     handle.unset_grab(data);
        //     return;
        // }

        // TODO: Impl resize using cursor
        // let (mut dx, mut dy) = (event.location - self.start_data.location).into();
        //
        // let mut new_window_width = self.initial_window_size.w;
        // let mut new_window_height = self.initial_window_size.h;
        //
        // let left_right = ResizeEdge::LEFT | ResizeEdge::RIGHT;
        // let top_bottom = ResizeEdge::TOP | ResizeEdge::BOTTOM;
        //
        // if self.edges.intersects(left_right) {
        //     if self.edges.intersects(ResizeEdge::LEFT) {
        //         dx = -dx;
        //     }
        //
        //     new_window_width = (self.initial_window_size.w as f64 + dx) as i32;
        // }
        //
        // if self.edges.intersects(top_bottom) {
        //     if self.edges.intersects(ResizeEdge::TOP) {
        //         dy = -dy;
        //     }
        //
        //     new_window_height = (self.initial_window_size.h as f64 + dy) as i32;
        // }
        //
        // let (min_size, max_size) = if let Some(surface) = self.window.wl_surface() {
        //     with_states(&surface, |states| {
        //         let data = states.cached_state.current::<SurfaceCachedState>();
        //         (data.min_size, data.max_size)
        //     })
        // } else {
        //     ((0, 0).into(), (0, 0).into())
        // };
        //
        // let min_width = min_size.w.max(1);
        // let min_height = min_size.h.max(1);
        // let max_width = if max_size.w == 0 {
        //     i32::max_value()
        // } else {
        //     max_size.w
        // };
        // let max_height = if max_size.h == 0 {
        //     i32::max_value()
        // } else {
        //     max_size.h
        // };
        //
        // new_window_width = new_window_width.max(min_width).min(max_width);
        // new_window_height = new_window_height.max(min_height).min(max_height);
        //
        // self.last_window_size = (new_window_width, new_window_height).into();
        //
        // match self.window.0.underlying_surface() {
        //     WindowSurface::Wayland(xdg) => {
        //         xdg.with_pending_state(|state| {
        //             state.states.set(xdg_toplevel::State::Resizing);
        //             state.size = Some(self.last_window_size);
        //         });
        //         xdg.send_pending_configure();
        //     }
        //     WindowSurface::X11(x11) => {
        //         // TODO: find from which space this window is
        //         let location = data
        //             .spaces
        //             .values_mut()
        //             .next()
        //             .unwrap()
        //             .element_location(&self.window)
        //             .unwrap();
        //         x11.configure(Rectangle::from_loc_and_size(
        //             location,
        //             self.last_window_size,
        //         ))
        //         .unwrap();
        //     }
        // }
    }

    fn frame(
        &mut self,
        _data: &mut State,
        _handle: &mut smithay::input::touch::TouchInnerHandle<'_, State>,
        _seq: Serial,
    ) {
    }

    fn cancel(
        &mut self,
        data: &mut State,
        handle: &mut smithay::input::touch::TouchInnerHandle<'_, State>,
        seq: Serial,
    ) {
        handle.cancel(data, seq);
        handle.unset_grab(self, data);
    }

    fn start_data(&self) -> &smithay::input::touch::GrabStartData<State> {
        &self.start_data
    }

    fn shape(
        &mut self,
        data: &mut State,
        handle: &mut smithay::input::touch::TouchInnerHandle<'_, State>,
        event: &ShapeEvent,
        seq: Serial,
    ) {
        handle.shape(data, event, seq);
    }

    fn orientation(
        &mut self,
        data: &mut State,
        handle: &mut smithay::input::touch::TouchInnerHandle<'_, State>,
        event: &OrientationEvent,
        seq: Serial,
    ) {
        handle.orientation(data, event, seq);
    }

    fn unset(&mut self, _data: &mut State) {}
}
