use crate::drawing::PointerRenderElement;
use smithay::desktop::utils::send_frames_surface_tree;
use smithay::output::Output;
use smithay::{backend::allocator::Fourcc, utils::Transform};
use smithay::{
    backend::renderer::{
        element::{
            memory::{MemoryRenderBuffer, MemoryRenderBufferRenderElement},
            AsRenderElements, Kind,
        },
        ImportAll, ImportMem, Renderer, Texture,
    },
    input::pointer::{CursorIcon, CursorImageStatus},
    utils::{Coordinate, Physical, Point, Scale},
};
use std::{collections::HashMap, io::Read, time::Duration};
use xcursor::{
    parser::{parse_xcursor, Image},
    CursorTheme,
};

static FALLBACK_CURSOR_DATA: &[u8] = include_bytes!("../resources/cursor.rgba");

#[derive(Debug)]
pub struct CursorState {
    cursor_theme: CursorTheme,
    status: CursorImageStatus,
    images: HashMap<CursorIcon, Vec<Image>>,
    size: u32,
    scale: Scale<f64>,
    pointer_images: Vec<(Image, MemoryRenderBuffer)>,
    buffer: Option<MemoryRenderBuffer>,
    time: Duration,
    nearest_images_loaded: bool,
    nearest_images: Vec<Image>,
    total_delay: u32,
}

impl Default for CursorState {
    fn default() -> Self {
        let cursor_theme = Self::load_cursor_theme();
        let size = std::env::var("XCURSOR_SIZE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(24);

        let mut cursor_state = Self {
            cursor_theme,
            status: CursorImageStatus::default_named(),
            images: HashMap::new(),
            size,
            scale: Scale::from(1.0),
            pointer_images: Vec::new(),
            buffer: None,
            time: Duration::default(),
            nearest_images_loaded: false,
            nearest_images: Vec::new(),
            total_delay: 0,
        };

        cursor_state.load_icon(CursorIcon::Default);
        cursor_state
    }
}

impl CursorState {
    fn load_cursor_theme() -> CursorTheme {
        let theme = std::env::var("XCURSOR_THEME");
        let theme_name = theme.as_deref().ok().unwrap_or("default");

        CursorTheme::load(theme_name)
    }

    pub fn update_status(&mut self, status: CursorImageStatus) {
        self.status = status;
        if let CursorImageStatus::Named(icon) = &self.status {
            self.load_icon(*icon);
            self.nearest_images_loaded = false;
        } else {
            self.nearest_images_loaded = true;
        }
    }

    pub fn get_default_image(&mut self) -> Image {
        self.images[&CursorIcon::Default][0].clone()
    }

    fn load_icon(&mut self, icon: CursorIcon) {
        if self.images.contains_key(&icon) {
            return;
        }
        let frames = load_frames(&self.cursor_theme, icon).unwrap_or_else(load_fallback_frames);
        self.images.insert(icon, frames);
    }

    pub fn set_scale(&mut self, scale: Scale<f64>) {
        if self.scale != scale {
            self.scale = scale;
            self.nearest_images_loaded = false;
        }

        if !self.nearest_images_loaded {
            if let CursorImageStatus::Named(icon) = &self.status {
                self.nearest_images = nearest_images(
                    // TODO: don't only consider x field
                    (self.size.to_f64() * self.scale.to_f64().x) as u32,
                    &self.images[icon],
                );
                self.total_delay = self
                    .nearest_images
                    .iter()
                    .fold(0, |acc, image| acc + image.delay);
            }

            self.nearest_images_loaded = true;
        }
    }

    pub fn set_time(&mut self, time: Duration) {
        self.time = time;

        if let CursorImageStatus::Named(_) = &self.status {
            let frame = frame(
                (self.time.as_millis() % self.total_delay as u128) as u32,
                &self.nearest_images,
            );

            let pointer_images = &mut self.pointer_images;
            let pointer_image = pointer_images
                .iter()
                .find_map(|(image, texture)| {
                    if image == &frame {
                        Some(texture.clone())
                    } else {
                        None
                    }
                })
                .unwrap_or_else(|| {
                    let buffer = MemoryRenderBuffer::from_slice(
                        &frame.pixels_rgba,
                        Fourcc::Argb8888,
                        (frame.width as i32, frame.height as i32),
                        1,
                        Transform::Normal,
                        None,
                    );
                    pointer_images.push((frame, buffer.clone()));
                    buffer
                });

            self.buffer = Some(pointer_image);
        }
    }

    pub fn status(&self) -> &CursorImageStatus {
        &self.status
    }

    pub fn send_frame<T>(&self, output: &Output, time: T)
    where
        T: Into<Duration>,
    {
        if let CursorImageStatus::Surface(surface) = &self.status {
            send_frames_surface_tree(surface, output, time, Some(Duration::ZERO), |_, _| None);
        }
    }
}

impl<T: Texture + Clone + 'static, R> AsRenderElements<R> for CursorState
where
    R: Renderer<TextureId = T> + ImportAll + ImportMem,
{
    type RenderElement = PointerRenderElement<R>;
    fn render_elements<E>(
        &self,
        renderer: &mut R,
        location: Point<i32, Physical>,
        scale: Scale<f64>,
        alpha: f32,
    ) -> Vec<E>
    where
        E: From<PointerRenderElement<R>>,
    {
        match &self.status {
            CursorImageStatus::Hidden => vec![],
            CursorImageStatus::Named(_) => {
                if let Some(buffer) = self.buffer.as_ref() {
                    vec![PointerRenderElement::<R>::from(
                        MemoryRenderBufferRenderElement::from_buffer(
                            renderer,
                            location.to_f64(),
                            buffer,
                            None,
                            None,
                            None,
                            Kind::Cursor,
                        )
                        .expect("Lost system pointer buffer"),
                    )
                    .into()]
                } else {
                    vec![]
                }
            }
            CursorImageStatus::Surface(surface) => {
                let elements: Vec<PointerRenderElement<R>> =
                    smithay::backend::renderer::element::surface::render_elements_from_surface_tree(
                        renderer,
                        surface,
                        location,
                        scale,
                        alpha,
                        Kind::Cursor,
                    );
                elements.into_iter().map(E::from).collect()
            }
        }
    }
}

fn load_frames(cursor_theme: &CursorTheme, icon: CursorIcon) -> Option<Vec<Image>> {
    let icon_path = cursor_theme.load_icon(icon.name())?;
    let mut cursor_file = std::fs::File::open(icon_path).ok()?;
    let mut cursor_data = Vec::new();
    cursor_file.read_to_end(&mut cursor_data).ok()?;
    parse_xcursor(&cursor_data)
}

fn load_fallback_frames() -> Vec<Image> {
    vec![Image {
        size: 32,
        width: 64,
        height: 64,
        xhot: 1,
        yhot: 1,
        delay: 1,
        pixels_rgba: Vec::from(FALLBACK_CURSOR_DATA),
        pixels_argb: vec![],
    }]
}

fn nearest_images(size: u32, images: &[Image]) -> Vec<Image> {
    // Follow the nominal size of the cursor to choose the nearest
    let nearest_image = images
        .iter()
        .min_by_key(|image| (size as i32 - image.size as i32).abs())
        .unwrap();

    images
        .iter()
        .filter(move |image| {
            image.width == nearest_image.width && image.height == nearest_image.height
        })
        .map(|image| image.to_owned())
        .collect()
}

fn frame(mut millis: u32, images: &[Image]) -> Image {
    for img in images {
        if millis < img.delay {
            return img.clone();
        }
        millis -= img.delay;
    }

    unreachable!()
}
