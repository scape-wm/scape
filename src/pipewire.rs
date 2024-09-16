use crate::{render::GlMultiRenderer, State};
use anyhow::anyhow;
use calloop::{generic::Generic, Interest, LoopHandle, Mode, PostAction};
use egui::ahash::HashMap;
use pipewire as pw;
use pw::{
    context::Context,
    core::Core,
    keys::{MEDIA_CATEGORY, MEDIA_ROLE, MEDIA_TYPE},
    loop_::LoopRef,
    main_loop::MainLoop,
    properties::properties,
    spa::{
        buffer::DataType,
        param::{
            format::{FormatProperties, MediaSubtype, MediaType},
            format_utils::parse_format,
            video::{VideoFormat, VideoInfoRaw},
            ParamType,
        },
        pod::{
            self, object, property, serialize::PodSerializer, ChoiceValue, Pod, Property,
            PropertyFlags, Value,
        },
        sys::{
            SPA_PARAM_BUFFERS_align, SPA_PARAM_BUFFERS_blocks, SPA_PARAM_BUFFERS_buffers,
            SPA_PARAM_BUFFERS_dataType, SPA_PARAM_BUFFERS_size, SPA_PARAM_BUFFERS_stride,
            SPA_DATA_FLAG_READWRITE,
        },
        utils::{Choice, ChoiceEnum, ChoiceFlags, Direction, SpaTypes},
    },
    stream::{Stream, StreamFlags, StreamListener, StreamRef, StreamState},
    sys::pw_buffer,
};
use smithay::{
    backend::{
        allocator::{
            dmabuf::{AsDmabuf, Dmabuf},
            gbm::GbmBuffer,
        },
        drm::{compositor::RenderFrameResult, gbm::GbmFramebuffer, DrmDeviceFd},
        renderer::{element::RenderElement, Bind},
    },
    output::Output,
    reexports::gbm::{BufferObjectFlags, Device as GbmDevice, Format as Fourcc, Modifier},
    utils::{Physical, Rectangle, Transform},
};
use std::{
    cell::RefCell,
    io::Cursor,
    os::fd::{AsFd, AsRawFd, BorrowedFd},
    rc::Rc,
    time::Duration,
};
use tracing::{error, info, warn};

struct MainLoopAsFd(MainLoop);

impl AsFd for MainLoopAsFd {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.loop_().fd()
    }
}

#[derive(Clone, Debug)]
pub enum CursorMode {
    Hidden,
    Embedded,
    Stream,
}

pub struct VideoStream {
    _listener: StreamListener<VideoStreamData>,
    dmabufs: Rc<RefCell<HashMap<i64, Dmabuf>>>,
    region: Rc<RefCell<Rectangle<i32, Physical>>>,
    stream: Stream,
}

impl std::fmt::Debug for VideoStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoStream")
            .field("dmabufs", &self.dmabufs)
            .field("stream", &self.stream)
            .finish()
    }
}

impl VideoStream {
    pub fn node_id(&self) -> u32 {
        self.stream.node_id()
    }

    pub fn render_frame<'a, 'b, E>(
        &self,
        renderer: &mut GlMultiRenderer<'a>,
        render_result: &RenderFrameResult<'b, GbmBuffer, GbmFramebuffer, E>,
        output: &Output,
    ) where
        E: RenderElement<GlMultiRenderer<'a>>,
    {
        if !matches!(self.stream.state(), StreamState::Streaming) {
            return;
        }

        let Some(mut buffer) = self.stream.dequeue_buffer() else {
            warn!("No buffer available");
            return;
        };

        let buffer_data = &mut buffer.datas_mut()[0];
        let fd = buffer_data.as_raw().fd;
        let Some(dmabuf) = self.dmabufs.borrow().get(&fd).cloned() else {
            error!(fd, "Unable to find dmabuf");
            return;
        };

        if let Err(err) = renderer.bind(dmabuf) {
            error!(?err, "Unable to bind dmabuf");
            return;
        }

        let scale = output.current_scale().fractional_scale();
        let output_size = output.current_mode().unwrap().size;
        let transform = output.current_transform();

        // Calculate drawing area after output transform.
        let region = self.region.borrow();
        let damage = transform.transform_rect_in(*region, &output_size);
        if let Err(err) = render_result.blit_frame_result(
            damage.size,
            Transform::Normal,
            scale,
            renderer,
            [damage],
            [],
        ) {
            error!(?err, "Unable to render frame");
        }

        let maxsize = buffer_data.as_raw().maxsize;
        let chunk = buffer_data.chunk_mut();
        *chunk.size_mut() = maxsize;
        *chunk.stride_mut() = maxsize as i32 / region.size.h;
    }
}

struct VideoStreamData {
    gbm: GbmDevice<DrmDeviceFd>,
    dmabufs: Rc<RefCell<HashMap<i64, Dmabuf>>>,
    width: u32,
    height: u32,
    format: Fourcc,
    size: u32,
    region: Rc<RefCell<Rectangle<i32, Physical>>>,
}

#[derive(Debug)]
pub struct Pipewire {
    // hold on to the context, which also holds on to the main loop
    _context: Context,
    core: Core,
}

impl Pipewire {
    pub fn new(loop_handle: LoopHandle<'static, State>) -> anyhow::Result<Self> {
        let main_loop = MainLoop::new(None)?;
        let context = Context::new(&main_loop)?;
        let core = context.connect(None)?;

        loop_handle
            .insert_source(
                Generic::new(MainLoopAsFd(main_loop), Interest::READ, Mode::Level),
                |_, main_loop, _| {
                    run_pipewire_iteration(main_loop.0.loop_());
                    Ok(PostAction::Continue)
                },
            )
            .map_err(|e| anyhow!("Unable to start pipewire main loop: {}", e))?;

        Ok(Self {
            _context: context,
            core,
        })
    }

    pub fn start_video_stream(&self, gbm: GbmDevice<DrmDeviceFd>) -> anyhow::Result<VideoStream> {
        pipewire::init();

        let stream = Stream::new(
            &self.core,
            "scape-video",
            properties! {
                *MEDIA_TYPE => "Video",
                *MEDIA_CATEGORY => "Playback",
                *MEDIA_ROLE => "Screen",
            },
        )?;
        let dmabufs = Rc::new(RefCell::new(HashMap::default()));
        let region = Rc::new(RefCell::new(Rectangle::from_loc_and_size((0, 0), (0, 0))));
        let listener = stream
            .add_local_listener_with_user_data(VideoStreamData {
                gbm,
                width: 0,
                height: 0,
                format: Fourcc::Argb8888,
                size: 0,
                dmabufs: dmabufs.clone(),
                region: region.clone(),
            })
            .state_changed(state_changed)
            .param_changed(param_changed)
            .add_buffer(add_buffer)
            .remove_buffer(remove_buffer)
            .process(process)
            .register()?;

        let obj = object!(
            SpaTypes::ObjectParamFormat,
            ParamType::EnumFormat,
            property!(FormatProperties::MediaType, Id, MediaType::Video),
            property!(FormatProperties::MediaSubtype, Id, MediaSubtype::Raw),
            property!(FormatProperties::VideoFormat, Id, VideoFormat::RGBA),
            // property!(
            //     FormatProperties::VideoFormat,
            //     Choice,
            //     Enum,
            //     Id,
            //     VideoFormat::RGB,
            //     VideoFormat::RGB,
            //     VideoFormat::RGBx,
            //     VideoFormat::BGRx,
            // ),
            Property {
                key: FormatProperties::VideoModifier.as_raw(),
                value: Value::Long(u64::from(Modifier::Invalid) as i64),
                flags: PropertyFlags::MANDATORY,
            },
            property!(
                FormatProperties::VideoSize,
                Rectangle,
                pw::spa::utils::Rectangle {
                    width: 1024,
                    height: 1024,
                }
            ),
            // property!(
            //     FormatProperties::VideoSize,
            //     Choice,
            //     Range,
            //     Rectangle,
            //     pw::spa::utils::Rectangle {
            //         width: 320,
            //         height: 240
            //     },
            //     pw::spa::utils::Rectangle {
            //         width: 1,
            //         height: 1
            //     },
            //     pw::spa::utils::Rectangle {
            //         width: 1024,
            //         height: 1024
            //     }
            // ),
            property!(
                FormatProperties::VideoFramerate,
                Choice,
                Range,
                Fraction,
                pw::spa::utils::Fraction { num: 25, denom: 1 },
                pw::spa::utils::Fraction { num: 0, denom: 1 },
                pw::spa::utils::Fraction {
                    num: 1000,
                    denom: 1
                }
            ),
            pod::property!(
                FormatProperties::VideoMaxFramerate,
                Choice,
                Range,
                Fraction,
                pw::spa::utils::Fraction {
                    num: 60,
                    denom: 1000
                },
                pw::spa::utils::Fraction { num: 1, denom: 1 },
                pw::spa::utils::Fraction {
                    num: 240,
                    denom: 1000
                }
            ),
        );
        let values: Vec<u8> =
            PodSerializer::serialize(Cursor::new(Vec::new()), &Value::Object(obj))
                .unwrap()
                .0
                .into_inner();

        let mut params = [Pod::from_bytes(&values).unwrap()];

        stream.connect(
            Direction::Output,
            None,
            StreamFlags::DRIVER | StreamFlags::ALLOC_BUFFERS,
            &mut params,
        )?;
        Ok(VideoStream {
            _listener: listener,
            dmabufs,
            stream,
            region,
        })
    }
}

#[cfg_attr(feature = "profiling", profiling::function)]
fn run_pipewire_iteration(main_loop: &LoopRef) {
    main_loop.iterate(Duration::ZERO);
}

fn process(_stream: &StreamRef, _data: &mut VideoStreamData) {
    info!("process");
}

fn state_changed(
    _stream: &StreamRef,
    _data: &mut VideoStreamData,
    old: StreamState,
    new: StreamState,
) {
    info!(?old, ?new, "state_changed");
}

fn add_buffer(_stream: &StreamRef, data: &mut VideoStreamData, pw_buffer: *mut pw_buffer) {
    info!("add_buffer");
    unsafe {
        let buffer = (*pw_buffer).buffer;
        let buffer_datas = (*buffer).datas;

        let buffer_object = match data.gbm.create_buffer_object::<()>(
            data.width,
            data.height,
            data.format,
            BufferObjectFlags::RENDERING | BufferObjectFlags::LINEAR,
        ) {
            Ok(buffer_object) => buffer_object,
            Err(err) => {
                error!(?err, "Unable to create gbm buffer object");
                return;
            }
        };

        let gbm_buffer = GbmBuffer::from_bo(buffer_object, true);
        let dmabuf = match gbm_buffer.export() {
            Ok(dmabuf) => dmabuf,
            Err(err) => {
                error!(?err, "Unable to export gbm buffer");
                return;
            }
        };

        (*buffer_datas).type_ = DataType::DmaBuf.as_raw();
        // TODO: Find out how to use the fd of the correct plain
        (*buffer_datas).fd = dmabuf.handles().next().unwrap().as_raw_fd() as i64;
        (*buffer_datas).maxsize = data.size;
        (*buffer_datas).flags = SPA_DATA_FLAG_READWRITE;

        data.dmabufs.borrow_mut().insert((*buffer_datas).fd, dmabuf);
    }
}

fn remove_buffer(_stream: &StreamRef, data: &mut VideoStreamData, pw_buffer: *mut pw_buffer) {
    info!("remove_buffer");
    unsafe {
        let buffer = (*pw_buffer).buffer;
        let buffer_datas = (*buffer).datas;

        data.dmabufs.borrow_mut().remove(&(*buffer_datas).fd);
    }
}

fn param_changed(stream: &StreamRef, data: &mut VideoStreamData, id: u32, param: Option<&Pod>) {
    let Some(param) = param else {
        error!("Param changed without param");
        return;
    };

    if ParamType::from_raw(id) != ParamType::Format {
        error!(id, "Unknown param type");
        return;
    };

    let Ok((media_type, media_sub_type)) = parse_format(param) else {
        error!("Unable to parse format");
        return;
    };

    if media_type != MediaType::Video {
        error!(?media_type, "Not a video format");
        return;
    }

    if media_sub_type != MediaSubtype::Raw {
        error!(?media_sub_type, "Not a raw format");
        return;
    }

    info!(?media_type, ?media_sub_type, "Video stream format changed");

    let mut video_info = VideoInfoRaw::new();
    if let Err(parse_err) = video_info.parse(param) {
        error!(?parse_err, "Unable to parse video format");
        return;
    }

    let (bytes_per_pixel, format) = match video_info.format() {
        VideoFormat::RGB => (3, Fourcc::Rgb888),
        VideoFormat::RGBx => (4, Fourcc::Rgba8888),
        VideoFormat::BGRx => (4, Fourcc::Bgra8888),
        _ => {
            error!(video_format = ?video_info.format(), "Unsupported video format");
            return;
        }
    };

    let video_size = video_info.size();

    let stride = video_size.width * bytes_per_pixel;
    let size = video_size.height * stride;

    // TODO: check why the property! macro does not work here
    let obj = object!(
        SpaTypes::ObjectParamBuffers,
        ParamType::Buffers,
        Property::new(
            SPA_PARAM_BUFFERS_buffers,
            Value::Choice(ChoiceValue::Int(Choice(
                ChoiceFlags::empty(),
                ChoiceEnum::Range {
                    default: 16,
                    min: 2,
                    max: 16
                }
            ))),
        ),
        Property::new(SPA_PARAM_BUFFERS_blocks, Value::Int(1)),
        Property::new(SPA_PARAM_BUFFERS_size, Value::Int(size as i32)),
        Property::new(SPA_PARAM_BUFFERS_stride, Value::Int(stride as i32)),
        // TODO: check if align can be something else than 16
        Property::new(SPA_PARAM_BUFFERS_align, Value::Int(16)),
        Property::new(
            SPA_PARAM_BUFFERS_dataType,
            Value::Choice(ChoiceValue::Int(Choice(
                ChoiceFlags::empty(),
                ChoiceEnum::Flags {
                    default: 1 << DataType::DmaBuf.as_raw(),
                    flags: vec![1 << DataType::DmaBuf.as_raw()],
                },
            ))),
        ),
    );

    let values: Vec<u8> = PodSerializer::serialize(Cursor::new(Vec::new()), &Value::Object(obj))
        .unwrap()
        .0
        .into_inner();
    let mut params = [Pod::from_bytes(&values).unwrap()];
    stream.update_params(&mut params).unwrap();

    data.width = video_size.width;
    data.height = video_size.height;
    data.format = format;
    data.size = size;
    *data.region.borrow_mut() =
        Rectangle::from_loc_and_size((0, 0), (video_size.width as i32, video_size.height as i32));
}
