use crate::{
    application_window::{ApplicationWindow, WindowRenderElement},
    egui_window::EguiWindow,
    focus::PointerFocusTarget,
    render::{AsGlowFrame, AsGlowRenderer, GlMultiError, GlMultiFrame, GlMultiRenderer},
};
use smithay::{
    backend::renderer::{
        element::{
            texture::TextureRenderElement, AsRenderElements, Element, Id, RenderElement,
            UnderlyingStorage,
        },
        gles::GlesTexture,
        glow::GlowRenderer,
        utils::{CommitCounter, DamageSet},
        ImportAll, ImportMem, Renderer,
    },
    desktop::{space::SpaceElement, WindowSurfaceType},
    output::Output,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Buffer, IsAlive, Logical, Physical, Point, Rectangle, Scale, Size},
    wayland::shell::xdg::ToplevelSurface,
    xwayland::X11Surface,
};

#[derive(Debug, Clone, PartialEq)]
pub enum WorkspaceWindow {
    ApplicationWindow(ApplicationWindow),
    EguiWindow(EguiWindow),
}

impl WorkspaceWindow {
    /// Sends a close request to the window. Depending on the window type, the window
    /// can be unmapped immediately. `true` is returned if the window needs to be unmapped
    /// and `false` if the window cannot be unmapped immediately.
    #[must_use]
    pub fn close(&self) -> bool {
        match self {
            WorkspaceWindow::ApplicationWindow(w) => {
                w.close();
                false
            }
            WorkspaceWindow::EguiWindow(_) => true,
        }
    }

    pub fn app_id(&self) -> String {
        match self {
            WorkspaceWindow::ApplicationWindow(w) => w.app_id(),
            WorkspaceWindow::EguiWindow(w) => w.app_id(),
        }
    }

    pub fn position(
        &self,
        location: Point<i32, Logical>,
        size: Size<i32, Logical>,
        bounds: Size<i32, Logical>,
        send_configure: bool,
    ) {
        match self {
            WorkspaceWindow::ApplicationWindow(w) => {
                w.position(location, size, bounds, send_configure)
            }
            WorkspaceWindow::EguiWindow(w) => w.position(size),
        }
    }

    pub fn resize(&self, location: Point<i32, Logical>, size: Size<i32, Logical>) {
        match self {
            WorkspaceWindow::ApplicationWindow(w) => w.resize(location, size),
            WorkspaceWindow::EguiWindow(w) => w.position(size),
        }
    }

    pub fn wl_surface(&self) -> Option<WlSurface> {
        match self {
            WorkspaceWindow::ApplicationWindow(w) => w.wl_surface(),
            WorkspaceWindow::EguiWindow(_) => None,
        }
    }

    pub fn set_ssd(&self, ssd: bool) {
        if let WorkspaceWindow::ApplicationWindow(w) = self {
            w.set_ssd(ssd)
        }
    }

    pub fn on_commit(&self) {
        if let WorkspaceWindow::ApplicationWindow(w) = self {
            w.on_commit()
        }
    }

    pub fn toplevel(&self) -> Option<&ToplevelSurface> {
        match self {
            WorkspaceWindow::ApplicationWindow(w) => w.toplevel(),
            _ => None,
        }
    }

    pub fn x11_surface(&self) -> Option<&X11Surface> {
        match self {
            WorkspaceWindow::ApplicationWindow(w) => w.x11_surface(),
            _ => None,
        }
    }

    pub fn surface_under(
        &self,
        position: Point<f64, Logical>,
        window_type: WindowSurfaceType,
    ) -> Option<(PointerFocusTarget, Point<i32, Logical>)> {
        match self {
            WorkspaceWindow::ApplicationWindow(w) => w.surface_under(position, window_type),
            _ => None,
        }
    }
}

impl IsAlive for WorkspaceWindow {
    fn alive(&self) -> bool {
        match self {
            WorkspaceWindow::ApplicationWindow(w) => w.alive(),
            WorkspaceWindow::EguiWindow(w) => w.alive(),
        }
    }
}

impl SpaceElement for WorkspaceWindow {
    fn bbox(&self) -> Rectangle<i32, Logical> {
        match self {
            WorkspaceWindow::ApplicationWindow(w) => w.bbox(),
            WorkspaceWindow::EguiWindow(w) => w.bbox(),
        }
    }

    fn is_in_input_region(&self, point: &Point<f64, Logical>) -> bool {
        match self {
            WorkspaceWindow::ApplicationWindow(w) => w.is_in_input_region(point),
            WorkspaceWindow::EguiWindow(w) => w.is_in_input_region(point),
        }
    }

    fn set_activate(&self, activated: bool) {
        match self {
            WorkspaceWindow::ApplicationWindow(w) => w.set_activate(activated),
            WorkspaceWindow::EguiWindow(w) => w.set_activate(activated),
        }
    }

    fn output_enter(&self, output: &Output, overlap: Rectangle<i32, Logical>) {
        match self {
            WorkspaceWindow::ApplicationWindow(w) => w.output_enter(output, overlap),
            WorkspaceWindow::EguiWindow(w) => w.output_enter(output, overlap),
        }
    }

    fn output_leave(&self, output: &Output) {
        match self {
            WorkspaceWindow::ApplicationWindow(w) => w.output_leave(output),
            WorkspaceWindow::EguiWindow(w) => w.output_leave(output),
        }
    }
}

pub enum WorkspaceWindowRenderElement<R>
where
    R: Renderer,
{
    ApplicationWindow(WindowRenderElement<R>),
    Egui(TextureRenderElement<GlesTexture>),
}

impl<R> Element for WorkspaceWindowRenderElement<R>
where
    R: Renderer + ImportAll + ImportMem,
    <R as Renderer>::TextureId: 'static,
{
    fn id(&self) -> &Id {
        match self {
            Self::ApplicationWindow(elem) => elem.id(),
            Self::Egui(elem) => elem.id(),
        }
    }

    fn current_commit(&self) -> CommitCounter {
        match self {
            Self::ApplicationWindow(elem) => elem.current_commit(),
            Self::Egui(elem) => elem.current_commit(),
        }
    }

    fn src(&self) -> Rectangle<f64, Buffer> {
        match self {
            Self::ApplicationWindow(elem) => elem.src(),
            Self::Egui(elem) => elem.src(),
        }
    }

    fn geometry(&self, scale: Scale<f64>) -> Rectangle<i32, Physical> {
        match self {
            Self::ApplicationWindow(elem) => elem.geometry(scale),
            Self::Egui(elem) => elem.geometry(scale),
        }
    }

    fn location(&self, scale: Scale<f64>) -> Point<i32, Physical> {
        match self {
            Self::ApplicationWindow(elem) => elem.location(scale),
            Self::Egui(elem) => elem.location(scale),
        }
    }

    fn transform(&self) -> smithay::utils::Transform {
        match self {
            Self::ApplicationWindow(elem) => elem.transform(),
            Self::Egui(elem) => elem.transform(),
        }
    }

    fn damage_since(
        &self,
        scale: Scale<f64>,
        commit: Option<CommitCounter>,
    ) -> DamageSet<i32, Physical> {
        match self {
            Self::ApplicationWindow(elem) => elem.damage_since(scale, commit),
            Self::Egui(elem) => elem.damage_since(scale, commit),
        }
    }

    fn opaque_regions(&self, scale: Scale<f64>) -> Vec<Rectangle<i32, Physical>> {
        match self {
            Self::ApplicationWindow(elem) => elem.opaque_regions(scale),
            Self::Egui(elem) => elem.opaque_regions(scale),
        }
    }

    fn alpha(&self) -> f32 {
        match self {
            Self::ApplicationWindow(elem) => elem.alpha(),
            Self::Egui(elem) => elem.alpha(),
        }
    }
}

impl<'a> RenderElement<GlMultiRenderer<'a>> for WorkspaceWindowRenderElement<GlMultiRenderer<'a>> {
    fn draw<'frame>(
        &self,
        frame: &mut GlMultiFrame<'a, 'frame>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
    ) -> Result<(), GlMultiError> {
        match self {
            Self::ApplicationWindow(elem) => elem.draw(frame, src, dst, damage),
            Self::Egui(elem) => {
                let glow_frame = frame.glow_frame_mut();
                RenderElement::<GlowRenderer>::draw(elem, glow_frame, src, dst, damage)
                    .map_err(GlMultiError::Render)
            }
        }
    }

    fn underlying_storage(&self, renderer: &mut GlMultiRenderer<'a>) -> Option<UnderlyingStorage> {
        match self {
            Self::ApplicationWindow(elem) => elem.underlying_storage(renderer),
            Self::Egui(elem) => {
                let glow_renderer = renderer.glow_renderer_mut();
                match elem.underlying_storage(glow_renderer) {
                    Some(UnderlyingStorage::Wayland(buffer)) => {
                        Some(UnderlyingStorage::Wayland(buffer))
                    }
                    _ => None,
                }
            }
        }
    }
}

impl RenderElement<GlowRenderer> for WorkspaceWindowRenderElement<GlowRenderer> {
    fn draw(
        &self,
        frame: &mut <GlowRenderer as Renderer>::Frame<'_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
    ) -> Result<(), <GlowRenderer as Renderer>::Error> {
        match self {
            Self::ApplicationWindow(elem) => elem.draw(frame, src, dst, damage),
            Self::Egui(elem) => RenderElement::<GlowRenderer>::draw(elem, frame, src, dst, damage),
        }
    }

    fn underlying_storage(&self, renderer: &mut GlowRenderer) -> Option<UnderlyingStorage> {
        match self {
            Self::ApplicationWindow(elem) => elem.underlying_storage(renderer),
            Self::Egui(elem) => {
                let glow_renderer = renderer.glow_renderer_mut();
                match elem.underlying_storage(glow_renderer) {
                    Some(UnderlyingStorage::Wayland(buffer)) => {
                        Some(UnderlyingStorage::Wayland(buffer))
                    }
                    _ => None,
                }
            }
        }
    }
}

impl<R: Renderer> std::fmt::Debug for WorkspaceWindowRenderElement<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ApplicationWindow(arg0) => {
                f.debug_tuple("ApplicationWindow").field(arg0).finish()
            }
            Self::Egui(arg0) => f.debug_tuple("Egui").field(arg0).finish(),
        }
    }
}

impl<R> From<WindowRenderElement<R>> for WorkspaceWindowRenderElement<R>
where
    R: Renderer,
{
    fn from(value: WindowRenderElement<R>) -> Self {
        WorkspaceWindowRenderElement::ApplicationWindow(value)
    }
}

impl<R> From<TextureRenderElement<GlesTexture>> for WorkspaceWindowRenderElement<R>
where
    R: Renderer,
{
    fn from(value: TextureRenderElement<GlesTexture>) -> Self {
        WorkspaceWindowRenderElement::Egui(value)
    }
}

impl<R> AsRenderElements<R> for WorkspaceWindow
where
    R: Renderer + ImportAll + ImportMem + AsGlowRenderer,
    <R as Renderer>::TextureId: 'static,
    WorkspaceWindowRenderElement<R>: RenderElement<R>,
{
    type RenderElement = WorkspaceWindowRenderElement<R>;

    fn render_elements<C: From<Self::RenderElement>>(
        &self,
        renderer: &mut R,
        location: Point<i32, Physical>,
        scale: Scale<f64>,
        alpha: f32,
    ) -> Vec<C> {
        match self {
            WorkspaceWindow::ApplicationWindow(w) => w
                .render_elements(renderer, location, scale, alpha)
                .into_iter()
                .map(C::from)
                .collect(),
            WorkspaceWindow::EguiWindow(w) => w
                .render_elements(renderer.glow_renderer_mut(), location, scale, alpha)
                .into_iter()
                .map(C::from)
                .collect(),
        }
    }
}

impl From<ApplicationWindow> for WorkspaceWindow {
    fn from(value: ApplicationWindow) -> Self {
        WorkspaceWindow::ApplicationWindow(value)
    }
}

impl From<EguiWindow> for WorkspaceWindow {
    fn from(value: EguiWindow) -> Self {
        WorkspaceWindow::EguiWindow(value)
    }
}
