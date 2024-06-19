use pipewire as pw;
use pipewire::{
    context::Context,
    keys::{MEDIA_CATEGORY, MEDIA_ROLE, MEDIA_TYPE},
    main_loop::MainLoop,
    properties::properties,
    spa::{pod::Pod, utils::Direction},
    stream::{Stream, StreamFlags, StreamRef, StreamState},
};
use pw::spa::param::format::FormatProperties;
use pw::spa::param::format::MediaSubtype;
use pw::spa::param::format::MediaType;
use pw::spa::param::video::VideoFormat;
use pw::spa::param::ParamType;
use pw::spa::pod::object;
use pw::spa::pod::property;
use pw::spa::utils::SpaTypes;
use pw::sys::pw_buffer;
use tracing::info;

struct Data {}

pub fn setup_video_steam() -> anyhow::Result<()> {
    pipewire::init();

    let main_loop = MainLoop::new(None)?;
    let context = Context::new(&main_loop)?;
    let core = context.connect(None)?;

    let stream = Stream::new(
        &core,
        "scape-video",
        properties! {
            *MEDIA_TYPE => "Video",
            *MEDIA_CATEGORY => "Playback",
            *MEDIA_ROLE => "Screen",
        },
    )?;
    let _listener = stream
        .add_local_listener_with_user_data(Data {})
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
        property!(
            FormatProperties::VideoFormat,
            Choice,
            Enum,
            Id,
            VideoFormat::RGB,
            VideoFormat::RGB,
            VideoFormat::RGBA,
            VideoFormat::RGBx,
            VideoFormat::BGRx,
            VideoFormat::YUY2,
            VideoFormat::I420,
        ),
        property!(
            FormatProperties::VideoSize,
            Choice,
            Range,
            Rectangle,
            pw::spa::utils::Rectangle {
                width: 320,
                height: 240
            },
            pw::spa::utils::Rectangle {
                width: 1,
                height: 1
            },
            pw::spa::utils::Rectangle {
                width: 4096,
                height: 4096
            }
        ),
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
    );
    let values: Vec<u8> = pw::spa::pod::serialize::PodSerializer::serialize(
        std::io::Cursor::new(Vec::new()),
        &pw::spa::pod::Value::Object(obj),
    )
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

    main_loop.run();

    Ok(())
}

fn process(_stream: &StreamRef, _data: &mut Data) {
    info!("process");
}

fn state_changed(_stream: &StreamRef, _data: &mut Data, old: StreamState, new: StreamState) {
    info!(?old, ?new, "state_changed");
}

fn add_buffer(_stream: &StreamRef, _data: &mut Data, _pw_buffer: *mut pw_buffer) {
    info!("add_buffer");
}

fn remove_buffer(_stream: &StreamRef, _data: &mut Data, _pw_buffer: *mut pw_buffer) {
    info!("remove_buffer");
}

fn param_changed(_stream: &StreamRef, _data: &mut Data, _id: u32, _param: Option<&Pod>) {
    info!("param_changed");
}
