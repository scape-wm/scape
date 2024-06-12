use std::sync::{Arc, Mutex};

use crate::{
    egui::{
        debug_ui::{DebugState, DebugUi},
        EguiState,
    },
    render::AsGlowRenderer,
    State,
};
use egui::Context;
use smithay::{
    backend::{
        input::KeyState,
        renderer::{
            element::{texture::TextureRenderElement, AsRenderElements},
            gles::GlesTexture,
            Renderer,
        },
    },
    desktop::space::SpaceElement,
    input::{
        keyboard::{KeyboardTarget, KeysymHandle, ModifiersState},
        pointer::{
            AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent,
            GesturePinchBeginEvent, GesturePinchEndEvent, GesturePinchUpdateEvent,
            GestureSwipeBeginEvent, GestureSwipeEndEvent, GestureSwipeUpdateEvent, MotionEvent,
            PointerTarget, RelativeMotionEvent,
        },
        Seat,
    },
    utils::{IsAlive, Logical, Physical, Point, Rectangle, Scale, Serial, Size},
};
use tracing::error;

#[derive(PartialEq, Debug, Clone)]
pub enum EguiAppState {
    DebugUi(DebugUi),
}

impl EguiAppState {
    fn udpate_ui(&mut self, ctx: &Context) {
        match self {
            EguiAppState::DebugUi(debug_ui) => debug_ui.show(ctx),
        }
    }

    pub fn app_id(&self) -> String {
        match self {
            EguiAppState::DebugUi(_) => "scape::debug_ui".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EguiWindow {
    egui_state: EguiState,
    app_state: Arc<Mutex<EguiAppState>>,
}

impl EguiWindow {
    pub fn new(app_state: impl Into<EguiAppState>) -> Self {
        let area = Rectangle::from_loc_and_size((0, 0), (500, 500));

        Self {
            egui_state: EguiState::new(area),
            app_state: Arc::new(Mutex::new(app_state.into())),
        }
    }

    pub fn update_debug_ui(&mut self, debug_state: DebugState) {
        match &mut *self.app_state.lock().unwrap() {
            EguiAppState::DebugUi(debug_ui) => debug_ui.update(debug_state),
        }
    }

    pub fn app_id(&self) -> String {
        self.app_state.lock().unwrap().app_id()
    }

    pub fn position(&self, size: Size<i32, Logical>) {
        self.egui_state.set_size(size);
    }
}

impl PartialEq for EguiWindow {
    fn eq(&self, other: &Self) -> bool {
        self.egui_state == other.egui_state
    }
}

impl IsAlive for EguiWindow {
    fn alive(&self) -> bool {
        self.egui_state.alive()
    }
}

impl SpaceElement for EguiWindow {
    fn bbox(&self) -> Rectangle<i32, smithay::utils::Logical> {
        self.egui_state.bbox()
    }

    fn is_in_input_region(
        &self,
        point: &smithay::utils::Point<f64, smithay::utils::Logical>,
    ) -> bool {
        self.egui_state.is_in_input_region(point)
    }

    fn set_activate(&self, activated: bool) {
        self.egui_state.set_activate(activated)
    }

    fn output_enter(
        &self,
        output: &smithay::output::Output,
        overlap: Rectangle<i32, smithay::utils::Logical>,
    ) {
        self.egui_state.output_enter(output, overlap)
    }

    fn output_leave(&self, output: &smithay::output::Output) {
        self.egui_state.output_leave(output)
    }
}

impl<R> AsRenderElements<R> for EguiWindow
where
    R: Renderer<TextureId = GlesTexture> + AsGlowRenderer,
{
    type RenderElement = TextureRenderElement<GlesTexture>;

    fn render_elements<C: From<Self::RenderElement>>(
        &self,
        renderer: &mut R,
        location: Point<i32, Physical>,
        scale: Scale<f64>,
        alpha: f32,
    ) -> Vec<C> {
        let render_result = self.egui_state.render(
            |ctx| self.app_state.lock().unwrap().udpate_ui(ctx),
            renderer.glow_renderer_mut(),
            // TODO: also consider scale.y
            location,
            scale.x,
            alpha,
        );

        match render_result {
            Ok(render_elements) => vec![C::from(render_elements)],
            Err(err) => {
                error!(?err, "Failed to render egui window");
                vec![]
            }
        }
    }
}

impl PointerTarget<State> for EguiWindow {
    fn enter(&self, seat: &Seat<State>, data: &mut State, event: &MotionEvent) {
        PointerTarget::enter(&self.egui_state, seat, data, event)
    }

    fn motion(&self, seat: &Seat<State>, data: &mut State, event: &MotionEvent) {
        PointerTarget::motion(&self.egui_state, seat, data, event)
    }

    fn relative_motion(&self, seat: &Seat<State>, data: &mut State, event: &RelativeMotionEvent) {
        PointerTarget::relative_motion(&self.egui_state, seat, data, event)
    }

    fn button(&self, seat: &Seat<State>, data: &mut State, event: &ButtonEvent) {
        PointerTarget::button(&self.egui_state, seat, data, event)
    }

    fn axis(&self, seat: &Seat<State>, data: &mut State, frame: AxisFrame) {
        PointerTarget::axis(&self.egui_state, seat, data, frame)
    }

    fn frame(&self, seat: &Seat<State>, data: &mut State) {
        PointerTarget::frame(&self.egui_state, seat, data)
    }

    fn gesture_swipe_begin(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        event: &GestureSwipeBeginEvent,
    ) {
        PointerTarget::gesture_swipe_begin(&self.egui_state, seat, data, event)
    }

    fn gesture_swipe_update(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        event: &GestureSwipeUpdateEvent,
    ) {
        PointerTarget::gesture_swipe_update(&self.egui_state, seat, data, event)
    }

    fn gesture_swipe_end(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        event: &GestureSwipeEndEvent,
    ) {
        PointerTarget::gesture_swipe_end(&self.egui_state, seat, data, event)
    }

    fn gesture_pinch_begin(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        event: &GesturePinchBeginEvent,
    ) {
        PointerTarget::gesture_pinch_begin(&self.egui_state, seat, data, event)
    }

    fn gesture_pinch_update(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        event: &GesturePinchUpdateEvent,
    ) {
        PointerTarget::gesture_pinch_update(&self.egui_state, seat, data, event)
    }

    fn gesture_pinch_end(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        event: &GesturePinchEndEvent,
    ) {
        PointerTarget::gesture_pinch_end(&self.egui_state, seat, data, event)
    }

    fn gesture_hold_begin(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        event: &GestureHoldBeginEvent,
    ) {
        PointerTarget::gesture_hold_begin(&self.egui_state, seat, data, event)
    }

    fn gesture_hold_end(&self, seat: &Seat<State>, data: &mut State, event: &GestureHoldEndEvent) {
        PointerTarget::gesture_hold_end(&self.egui_state, seat, data, event)
    }

    fn leave(&self, seat: &Seat<State>, data: &mut State, serial: Serial, time: u32) {
        PointerTarget::leave(&self.egui_state, seat, data, serial, time)
    }
}

impl KeyboardTarget<State> for EguiWindow {
    fn enter(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        keys: Vec<KeysymHandle<'_>>,
        serial: Serial,
    ) {
        KeyboardTarget::enter(&self.egui_state, seat, data, keys, serial)
    }

    fn leave(&self, seat: &Seat<State>, data: &mut State, serial: Serial) {
        KeyboardTarget::leave(&self.egui_state, seat, data, serial)
    }

    fn key(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        key: KeysymHandle<'_>,
        state: KeyState,
        serial: Serial,
        time: u32,
    ) {
        KeyboardTarget::key(&self.egui_state, seat, data, key, state, serial, time)
    }

    fn modifiers(
        &self,
        seat: &Seat<State>,
        data: &mut State,
        modifiers: ModifiersState,
        serial: Serial,
    ) {
        KeyboardTarget::modifiers(&self.egui_state, seat, data, modifiers, serial)
    }
}
