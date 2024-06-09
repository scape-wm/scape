use std::fmt::Debug;

#[cfg(feature = "debug")]
use crate::drawing::FpsElement;
use crate::drawing::{PointerRenderElement, CLEAR_COLOR};
use crate::{
    state::SessionLock,
    workspace_window::{WorkspaceWindow, WorkspaceWindowRenderElement},
};
use smithay::backend::renderer::element::{Element, Id, UnderlyingStorage};
use smithay::backend::renderer::glow::GlowFrame;
use smithay::backend::renderer::multigpu::MultiFrame;
use smithay::backend::renderer::utils::{CommitCounter, DamageSet};
use smithay::backend::renderer::Frame;
use smithay::utils::{Buffer, Physical, Transform};
use smithay::{
    backend::{
        drm::DrmDeviceFd,
        renderer::{
            damage::{Error as OutputDamageTrackerError, OutputDamageTracker, RenderOutputResult},
            element::{
                surface::{render_elements_from_surface_tree, WaylandSurfaceRenderElement},
                utils::{
                    ConstrainAlign, ConstrainScaleBehavior, CropRenderElement,
                    RelocateRenderElement, RescaleRenderElement,
                },
                Kind, RenderElement, Wrap,
            },
            glow::GlowRenderer,
            multigpu::{gbm::GbmGlesBackend, Error as MultiError, MultiRenderer},
            ImportAll, ImportMem, Renderer,
        },
    },
    desktop::space::{
        constrain_space_element, ConstrainBehavior, ConstrainReference, Space, SpaceRenderElements,
    },
    output::Output,
    utils::{Point, Rectangle, Scale, Size},
};

pub type GlMultiRenderer<'gpu> = MultiRenderer<
    'gpu,
    'gpu,
    GbmGlesBackend<GlowRenderer, DrmDeviceFd>,
    GbmGlesBackend<GlowRenderer, DrmDeviceFd>,
>;
pub type GlMultiFrame<'gpu, 'frame> = MultiFrame<
    'gpu,
    'gpu,
    'frame,
    GbmGlesBackend<GlowRenderer, DrmDeviceFd>,
    GbmGlesBackend<GlowRenderer, DrmDeviceFd>,
>;
pub type GlMultiError = MultiError<
    GbmGlesBackend<GlowRenderer, DrmDeviceFd>,
    GbmGlesBackend<GlowRenderer, DrmDeviceFd>,
>;

pub trait AsGlowRenderer
where
    Self: Renderer,
{
    fn glow_renderer(&self) -> &GlowRenderer;
    fn glow_renderer_mut(&mut self) -> &mut GlowRenderer;
}

impl AsGlowRenderer for GlowRenderer {
    fn glow_renderer(&self) -> &GlowRenderer {
        self
    }
    fn glow_renderer_mut(&mut self) -> &mut GlowRenderer {
        self
    }
}

impl<'a> AsGlowRenderer for GlMultiRenderer<'a> {
    fn glow_renderer(&self) -> &GlowRenderer {
        self.as_ref()
    }
    fn glow_renderer_mut(&mut self) -> &mut GlowRenderer {
        self.as_mut()
    }
}

pub trait AsGlowFrame<'a>
where
    Self: Frame,
{
    fn glow_frame(&self) -> &GlowFrame<'a>;
    fn glow_frame_mut(&mut self) -> &mut GlowFrame<'a>;
}

impl<'frame> AsGlowFrame<'frame> for GlowFrame<'frame> {
    fn glow_frame(&self) -> &GlowFrame<'frame> {
        self
    }
    fn glow_frame_mut(&mut self) -> &mut GlowFrame<'frame> {
        self
    }
}

impl<'renderer, 'frame> AsGlowFrame<'frame> for GlMultiFrame<'renderer, 'frame> {
    fn glow_frame(&self) -> &GlowFrame<'frame> {
        self.as_ref()
    }
    fn glow_frame_mut(&mut self) -> &mut GlowFrame<'frame> {
        self.as_mut()
    }
}

smithay::backend::renderer::element::render_elements! {
    pub CustomRenderElements<R> where R: ImportAll + ImportMem;
    Pointer=PointerRenderElement<R>,
    Surface=WaylandSurfaceRenderElement<R>,
    // Note: We would like to borrow this element instead, but that would introduce
    // a feature-dependent lifetime, which introduces a lot more feature bounds
    // as the whole type changes and we can't have an unused lifetime (for when "debug" is disabled)
    // in the declaration.
    #[cfg(feature = "debug")]
    Fps=FpsElement<<R as Renderer>::TextureId>,
}

impl<R: Renderer + Debug> Debug for CustomRenderElements<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pointer(arg0) => f.debug_tuple("Pointer").field(arg0).finish(),
            Self::Surface(arg0) => f.debug_tuple("Surface").field(arg0).finish(),
            #[cfg(feature = "debug")]
            Self::Fps(arg0) => f.debug_tuple("Fps").field(arg0).finish(),
            Self::_GenericCatcher(arg0) => f.debug_tuple("_GenericCatcher").field(arg0).finish(),
        }
    }
}

pub enum OutputRenderElements<R>
where
    R: Renderer + ImportAll + ImportMem,
    <R as Renderer>::TextureId: 'static,
    WorkspaceWindowRenderElement<R>: RenderElement<R>,
{
    Space(SpaceRenderElements<R, WorkspaceWindowRenderElement<R>>),
    Window(Wrap<WorkspaceWindowRenderElement<R>>),
    Custom(CustomRenderElements<R>),
    WaylandSurface(WaylandSurfaceRenderElement<R>),
    Preview(
        CropRenderElement<
            RelocateRenderElement<RescaleRenderElement<WorkspaceWindowRenderElement<R>>>,
        >,
    ),
}

impl<R> Element for OutputRenderElements<R>
where
    R: Renderer + ImportAll + ImportMem,
    <R as Renderer>::TextureId: 'static,
    WorkspaceWindowRenderElement<R>: RenderElement<R>,
{
    fn id(&self) -> &Id {
        match self {
            Self::Space(elem) => elem.id(),
            Self::Window(elem) => elem.id(),
            Self::Custom(elem) => elem.id(),
            Self::WaylandSurface(elem) => elem.id(),
            Self::Preview(elem) => elem.id(),
        }
    }

    fn current_commit(&self) -> CommitCounter {
        match self {
            Self::Space(elem) => elem.current_commit(),
            Self::Window(elem) => elem.current_commit(),
            Self::Custom(elem) => elem.current_commit(),
            Self::WaylandSurface(elem) => elem.current_commit(),
            Self::Preview(elem) => elem.current_commit(),
        }
    }

    fn src(&self) -> Rectangle<f64, Buffer> {
        match self {
            Self::Space(elem) => elem.src(),
            Self::Window(elem) => elem.src(),
            Self::Custom(elem) => elem.src(),
            Self::WaylandSurface(elem) => elem.src(),
            Self::Preview(elem) => elem.src(),
        }
    }

    fn geometry(&self, scale: Scale<f64>) -> Rectangle<i32, Physical> {
        match self {
            Self::Space(elem) => elem.geometry(scale),
            Self::Window(elem) => elem.geometry(scale),
            Self::Custom(elem) => elem.geometry(scale),
            Self::WaylandSurface(elem) => elem.geometry(scale),
            Self::Preview(elem) => elem.geometry(scale),
        }
    }

    fn location(&self, scale: Scale<f64>) -> Point<i32, Physical> {
        match self {
            Self::Space(elem) => elem.location(scale),
            Self::Window(elem) => elem.location(scale),
            Self::Custom(elem) => elem.location(scale),
            Self::WaylandSurface(elem) => elem.location(scale),
            Self::Preview(elem) => elem.location(scale),
        }
    }

    fn transform(&self) -> Transform {
        match self {
            Self::Space(elem) => elem.transform(),
            Self::Window(elem) => elem.transform(),
            Self::Custom(elem) => elem.transform(),
            Self::WaylandSurface(elem) => elem.transform(),
            Self::Preview(elem) => elem.transform(),
        }
    }

    fn damage_since(
        &self,
        scale: Scale<f64>,
        commit: Option<CommitCounter>,
    ) -> DamageSet<i32, Physical> {
        match self {
            Self::Space(elem) => elem.damage_since(scale, commit),
            Self::Window(elem) => elem.damage_since(scale, commit),
            Self::Custom(elem) => elem.damage_since(scale, commit),
            Self::WaylandSurface(elem) => elem.damage_since(scale, commit),
            Self::Preview(elem) => elem.damage_since(scale, commit),
        }
    }

    fn opaque_regions(&self, scale: Scale<f64>) -> Vec<Rectangle<i32, Physical>> {
        match self {
            Self::Space(elem) => elem.opaque_regions(scale),
            Self::Window(elem) => elem.opaque_regions(scale),
            Self::Custom(elem) => elem.opaque_regions(scale),
            Self::WaylandSurface(elem) => elem.opaque_regions(scale),
            Self::Preview(elem) => elem.opaque_regions(scale),
        }
    }

    fn alpha(&self) -> f32 {
        match self {
            Self::Space(elem) => elem.alpha(),
            Self::Window(elem) => elem.alpha(),
            Self::Custom(elem) => elem.alpha(),
            Self::WaylandSurface(elem) => elem.alpha(),
            Self::Preview(elem) => elem.alpha(),
        }
    }
}

// impl<'a> RenderElement<GlMultiRenderer<'a>> for OutputRenderElements<GlMultiRenderer<'a>> {
//     fn draw(
//         &self,
//         frame: &mut <GlMultiRenderer<'a> as Renderer>::Frame<'_>,
//         src: Rectangle<f64, Buffer>,
//         dst: Rectangle<i32, Physical>,
//         damage: &[Rectangle<i32, Physical>],
//     ) -> Result<(), <GlMultiRenderer<'a> as Renderer>::Error> {
//         match self {
//             Self::Space(elem) => elem.draw(frame, src, dst, damage),
//             Self::Window(elem) => elem.draw(frame, src, dst, damage),
//             Self::Custom(elem) => elem.draw(frame, src, dst, damage),
//             Self::WaylandSurface(elem) => elem.draw(frame, src, dst, damage),
//             Self::Preview(elem) => elem.draw(frame, src, dst, damage),
//         }
//     }
//
//     fn underlying_storage(&self, renderer: &mut GlMultiRenderer<'a>) -> Option<UnderlyingStorage> {
//         match self {
//             Self::Space(elem) => elem.underlying_storage(renderer),
//             Self::Window(elem) => elem.underlying_storage(renderer),
//             Self::Custom(elem) => elem.underlying_storage(renderer),
//             Self::WaylandSurface(elem) => elem.underlying_storage(renderer),
//             Self::Preview(elem) => elem.underlying_storage(renderer),
//         }
//     }
// }

impl<R> RenderElement<R> for OutputRenderElements<R>
where
    R: Renderer + ImportAll + ImportMem,
    <R as Renderer>::TextureId: 'static,
    WorkspaceWindowRenderElement<R>: RenderElement<R>,
{
    fn draw(
        &self,
        frame: &mut <R as Renderer>::Frame<'_>,
        src: Rectangle<f64, Buffer>,
        dst: Rectangle<i32, Physical>,
        damage: &[Rectangle<i32, Physical>],
    ) -> Result<(), <R as Renderer>::Error> {
        match self {
            Self::Space(elem) => elem.draw(frame, src, dst, damage),
            Self::Window(elem) => elem.draw(frame, src, dst, damage),
            Self::Custom(elem) => elem.draw(frame, src, dst, damage),
            Self::WaylandSurface(elem) => elem.draw(frame, src, dst, damage),
            Self::Preview(elem) => elem.draw(frame, src, dst, damage),
        }
    }

    fn underlying_storage(&self, renderer: &mut R) -> Option<UnderlyingStorage> {
        match self {
            Self::Space(elem) => elem.underlying_storage(renderer),
            Self::Window(elem) => elem.underlying_storage(renderer),
            Self::Custom(elem) => elem.underlying_storage(renderer),
            Self::WaylandSurface(elem) => elem.underlying_storage(renderer),
            Self::Preview(elem) => elem.underlying_storage(renderer),
        }
    }
}

impl<R> Debug for OutputRenderElements<R>
where
    R: Renderer + ImportAll + ImportMem,
    <R as Renderer>::TextureId: 'static,
    WorkspaceWindowRenderElement<R>: RenderElement<R> + Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Space(arg0) => f.debug_tuple("Space").field(arg0).finish(),
            Self::Window(arg0) => f.debug_tuple("Window").field(arg0).finish(),
            Self::Custom(arg0) => f.debug_tuple("Custom").field(arg0).finish(),
            Self::Preview(arg0) => f.debug_tuple("Preview").field(arg0).finish(),
            Self::WaylandSurface(arg0) => f.debug_tuple("WaylandSurface").field(arg0).finish(),
        }
    }
}

impl<R> From<SpaceRenderElements<R, WorkspaceWindowRenderElement<R>>> for OutputRenderElements<R>
where
    R: Renderer + ImportAll + ImportMem,
    <R as Renderer>::TextureId: 'static,
    WorkspaceWindowRenderElement<R>: RenderElement<R>,
{
    fn from(value: SpaceRenderElements<R, WorkspaceWindowRenderElement<R>>) -> Self {
        OutputRenderElements::Space(value)
    }
}

impl<R> From<Wrap<WorkspaceWindowRenderElement<R>>> for OutputRenderElements<R>
where
    R: Renderer + ImportAll + ImportMem,
    <R as Renderer>::TextureId: 'static,
    WorkspaceWindowRenderElement<R>: RenderElement<R>,
{
    fn from(value: Wrap<WorkspaceWindowRenderElement<R>>) -> Self {
        OutputRenderElements::Window(value)
    }
}

impl<R> From<CustomRenderElements<R>> for OutputRenderElements<R>
where
    R: Renderer + ImportAll + ImportMem,
    <R as Renderer>::TextureId: 'static,
    WorkspaceWindowRenderElement<R>: RenderElement<R>,
{
    fn from(value: CustomRenderElements<R>) -> Self {
        OutputRenderElements::Custom(value)
    }
}

impl<R> From<WaylandSurfaceRenderElement<R>> for OutputRenderElements<R>
where
    R: Renderer + ImportAll + ImportMem,
    <R as Renderer>::TextureId: 'static,
    WorkspaceWindowRenderElement<R>: RenderElement<R>,
{
    fn from(value: WaylandSurfaceRenderElement<R>) -> Self {
        OutputRenderElements::WaylandSurface(value)
    }
}

impl<R>
    From<
        CropRenderElement<
            RelocateRenderElement<RescaleRenderElement<WorkspaceWindowRenderElement<R>>>,
        >,
    > for OutputRenderElements<R>
where
    R: Renderer + ImportAll + ImportMem,
    <R as Renderer>::TextureId: 'static,
    WorkspaceWindowRenderElement<R>: RenderElement<R>,
{
    fn from(
        value: CropRenderElement<
            RelocateRenderElement<RescaleRenderElement<WorkspaceWindowRenderElement<R>>>,
        >,
    ) -> Self {
        OutputRenderElements::Preview(value)
    }
}

pub fn space_preview_elements<'a, R, C>(
    renderer: &'a mut R,
    space: &'a Space<WorkspaceWindow>,
    output: &'a Output,
) -> impl Iterator<Item = C> + 'a
where
    R: Renderer + ImportAll + ImportMem + AsGlowRenderer,
    <R as Renderer>::TextureId: 'static,
    WorkspaceWindowRenderElement<R>: RenderElement<R>,
    C: From<
            CropRenderElement<
                RelocateRenderElement<RescaleRenderElement<WorkspaceWindowRenderElement<R>>>,
            >,
        > + 'a,
{
    let constrain_behavior = ConstrainBehavior {
        reference: ConstrainReference::BoundingBox,
        behavior: ConstrainScaleBehavior::Fit,
        align: ConstrainAlign::CENTER,
    };

    const PREVIEW_PADDING: i32 = 10;

    let elements_on_space = space.elements_for_output(output).count();
    let output_scale = output.current_scale().fractional_scale();
    let output_transform = output.current_transform();
    let output_size = output
        .current_mode()
        .map(|mode| {
            output_transform
                .transform_size(mode.size)
                .to_f64()
                .to_logical(output_scale)
        })
        .unwrap_or_default();

    let max_elements_per_row = 4;
    let elements_per_row = usize::min(elements_on_space, max_elements_per_row);
    let rows = f64::ceil(elements_on_space as f64 / elements_per_row as f64);

    let preview_size = Size::from((
        f64::round(output_size.w / elements_per_row as f64) as i32 - PREVIEW_PADDING * 2,
        f64::round(output_size.h / rows) as i32 - PREVIEW_PADDING * 2,
    ));

    space
        .elements_for_output(output)
        .enumerate()
        .flat_map(move |(element_index, window)| {
            let column = element_index % elements_per_row;
            let row = element_index / elements_per_row;
            let preview_location = Point::from((
                PREVIEW_PADDING + (PREVIEW_PADDING + preview_size.w) * column as i32,
                PREVIEW_PADDING + (PREVIEW_PADDING + preview_size.h) * row as i32,
            ));
            let constrain = Rectangle::from_loc_and_size(preview_location, preview_size);
            constrain_space_element(
                renderer,
                window,
                preview_location,
                1.0,
                output_scale,
                constrain,
                constrain_behavior,
            )
        })
}

#[cfg_attr(feature = "profiling", profiling::function)]
pub fn output_elements<R>(
    output: &Output,
    space: &Space<WorkspaceWindow>,
    custom_elements: impl IntoIterator<Item = CustomRenderElements<R>>,
    renderer: &mut R,
    show_window_preview: bool,
    session_lock: &Option<SessionLock>,
) -> (Vec<OutputRenderElements<R>>, [f32; 4])
where
    R: Renderer + ImportAll + ImportMem + AsGlowRenderer,
    WorkspaceWindowRenderElement<R>: RenderElement<R>,
{
    if let Some(session_lock) = session_lock {
        return (
            session_lock_elements(renderer, output, session_lock),
            CLEAR_COLOR,
        );
    }

    let mut output_render_elements = custom_elements
        .into_iter()
        .map(OutputRenderElements::from)
        .collect::<Vec<_>>();

    if show_window_preview && space.elements_for_output(output).count() > 0 {
        output_render_elements.extend(space_preview_elements(renderer, space, output));
    }

    let space_elements = smithay::desktop::space::space_render_elements::<_, WorkspaceWindow, _>(
        renderer,
        [space],
        output,
        1.0,
    )
    .expect("output without mode?");
    output_render_elements.extend(space_elements.into_iter().map(OutputRenderElements::Space));

    (output_render_elements, CLEAR_COLOR)
}

fn session_lock_elements<R>(
    renderer: &mut R,
    output: &Output,
    session_lock: &SessionLock,
) -> Vec<OutputRenderElements<R>>
where
    R: Renderer + ImportAll + ImportMem,
    WorkspaceWindowRenderElement<R>: RenderElement<R>,
{
    if let Some(surface) = session_lock.surfaces.get(output) {
        let scale = Scale::from(output.current_scale().fractional_scale());
        render_elements_from_surface_tree(
            renderer,
            surface.wl_surface(),
            (0, 0),
            scale,
            1.0,
            Kind::Unspecified,
        )
    } else {
        Vec::new()
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render_output<'a, 'damage, R>(
    output: &'a Output,
    space: &'a Space<WorkspaceWindow>,
    custom_elements: impl IntoIterator<Item = CustomRenderElements<R>>,
    renderer: &'a mut R,
    damage_tracker: &'damage mut OutputDamageTracker,
    age: usize,
    show_window_preview: bool,
    session_lock: &Option<SessionLock>,
) -> Result<RenderOutputResult<'damage>, OutputDamageTrackerError<R>>
where
    R: Renderer + ImportAll + ImportMem + AsGlowRenderer,
    <R as Renderer>::TextureId: 'static,
    WorkspaceWindowRenderElement<R>: RenderElement<R>,
    OutputRenderElements<R>: RenderElement<R>,
{
    let (elements, clear_color) = output_elements(
        output,
        space,
        custom_elements,
        renderer,
        show_window_preview,
        session_lock,
    );
    damage_tracker.render_output(renderer, age, &elements, clear_color)
}
