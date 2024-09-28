use crate::dbus::portals::PortalResponse;
use crate::pipewire::CursorMode;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use tracing::warn;
use zbus::interface;
use zbus::zvariant::{
    DeserializeDict, ObjectPath, OwnedObjectPath, OwnedValue, SerializeDict, Type, Value,
};

use super::request::Request;
use super::session::{Session, SessionType};

pub static NODE_ID: AtomicU32 = AtomicU32::new(0);

#[derive(SerializeDict, DeserializeDict, Type, Debug, Default)]
#[zvariant(signature = "dict")]
struct CreateSessionResult {
    handle_token: String,
}

#[derive(Clone, SerializeDict, DeserializeDict, Default, Type, Debug)]
#[zvariant(signature = "dict")]
struct StreamProperties {
    id: Option<String>,
    position: Option<(i32, i32)>,
    size: Option<(i32, i32)>,
    source_type: Option<u32>,
}

#[derive(SerializeDict, DeserializeDict, Type, Debug, Default)]
#[zvariant(signature = "dict")]
pub struct SelectSourcesOptions {
    pub types: Option<u32>,
    pub multiple: Option<bool>,
    pub cursor_mode: Option<u32>,
    pub restore_token: Option<String>,
    pub persist_mode: Option<u32>,
}

const SOURCE_TYPE_MONITOR: u32 = 1;
const SOURCE_TYPE_WINDOW: u32 = 2;
const SOURCE_TYPE_VIRTUAL: u32 = 4;

#[derive(Clone, Debug)]
pub struct SourceTypes {
    pub monitor: bool,
    pub window: bool,
    pub virt: bool,
}

impl Default for SourceTypes {
    fn default() -> Self {
        Self {
            monitor: true,
            window: false,
            virt: false,
        }
    }
}

impl From<u32> for SourceTypes {
    fn from(value: u32) -> Self {
        Self {
            monitor: value & SOURCE_TYPE_MONITOR != 0,
            window: value & SOURCE_TYPE_WINDOW != 0,
            virt: value & SOURCE_TYPE_VIRTUAL != 0,
        }
    }
}

// TODO: implement correct handling of persist_mode
const PERSIST_MODE_DOT_NOT: u32 = 0;
const PERSIST_MODE_PERSIST_SESSION: u32 = 1;
const PERSIST_MODE_PERSIST_FOREVER: u32 = 2;

#[derive(Clone, Debug, Default)]
pub enum PersistMode {
    #[default]
    DoNot,
    PersistSession,
    PersistForever,
}

impl From<u32> for PersistMode {
    fn from(value: u32) -> Self {
        match value {
            PERSIST_MODE_DOT_NOT => Self::DoNot,
            PERSIST_MODE_PERSIST_SESSION => Self::PersistSession,
            PERSIST_MODE_PERSIST_FOREVER => Self::PersistForever,
            _ => Self::DoNot,
        }
    }
}

const CURSOR_MODE_HIDDEN: u32 = 1;
const CURSOR_MODE_EMBEDDED: u32 = 2;
const CURSOR_MODE_STREAM: u32 = 4;

#[derive(Clone, Debug)]
pub struct SourceOptions {
    pub types: SourceTypes,
    pub multiple: bool,
    pub cursor_mode: CursorMode,
    pub restore_token: Option<String>,
    pub persist_mode: PersistMode,
}

impl From<SelectSourcesOptions> for SourceOptions {
    fn from(value: SelectSourcesOptions) -> Self {
        Self {
            types: value.types.map(Into::into).unwrap_or_default(),
            multiple: value.multiple.unwrap_or_default(),
            cursor_mode: value
                .cursor_mode
                .map(|cursor_mode| match cursor_mode {
                    CURSOR_MODE_HIDDEN => CursorMode::Hidden,
                    CURSOR_MODE_EMBEDDED => CursorMode::Embedded,
                    CURSOR_MODE_STREAM => CursorMode::Stream,
                    _ => CursorMode::Hidden,
                })
                .unwrap_or(CursorMode::Hidden),
            restore_token: value.restore_token,
            persist_mode: value.persist_mode.map(Into::into).unwrap_or_default(),
        }
    }
}

#[derive(Clone, SerializeDict, DeserializeDict, Default, Debug, Type)]
#[zvariant(signature = "dict")]
struct StartResult {
    streams: Vec<(u32, StreamProperties)>,
    persist_mode: u32,
    restore_token: Option<String>,
}

pub type SessionSourceOptions = Arc<Mutex<Option<SourceOptions>>>;

#[derive(Default)]
pub struct ScreenCast {
    sessions: Arc<Mutex<HashMap<OwnedObjectPath, SessionSourceOptions>>>,
}

#[interface(name = "org.freedesktop.impl.portal.ScreenCast")]
impl ScreenCast {
    async fn create_session(
        &self,
        request_handle: ObjectPath<'_>,
        session_handle: ObjectPath<'_>,
        _app_id: String,
        // TODO: Is there something useful in the options?
        _options: HashMap<String, Value<'_>>,
        #[zbus(object_server)] server: &zbus::ObjectServer,
    ) -> zbus::fdo::Result<PortalResponse<CreateSessionResult>> {
        server
            .at(
                request_handle.clone(),
                Request {
                    handle_path: request_handle.clone().into(),
                },
            )
            .await?;
        let session_source_options = Arc::new(Mutex::new(None));
        let session = Session::new(
            session_handle.clone(),
            SessionType::ScreenCast,
            session_source_options,
        );
        server.at(session_handle.clone(), session).await?;
        Ok(PortalResponse::Success(CreateSessionResult {
            handle_token: session_handle.to_string(),
        }))
    }

    async fn select_sources(
        &self,
        _request_handle: ObjectPath<'_>,
        session_handle: ObjectPath<'_>,
        _app_id: String,
        options: SelectSourcesOptions,
    ) -> zbus::fdo::Result<PortalResponse<HashMap<String, OwnedValue>>> {
        let Ok(mut locked_sessions) = self.sessions.lock() else {
            return Ok(PortalResponse::Cancelled);
        };
        let Some(current_options) = locked_sessions.get_mut(&session_handle.into()) else {
            warn!("Trying to set options for non-existent session");
            return Ok(PortalResponse::Other);
        };

        let Ok(mut locked_options) = current_options.lock() else {
            return Ok(PortalResponse::Cancelled);
        };
        *locked_options = Some(options.into());

        // TODO: Is there anything to return?
        Ok(PortalResponse::Success(HashMap::new()))
    }

    async fn start(
        &self,
        _request_handle: ObjectPath<'_>,
        _session_handle: ObjectPath<'_>,
        _app_id: String,
        _parent_window: String,
        _options: HashMap<String, Value<'_>>,
    ) -> zbus::fdo::Result<PortalResponse<StartResult>> {
        Ok(PortalResponse::Success(StartResult {
            streams: vec![(NODE_ID.load(Ordering::SeqCst), StreamProperties::default())],
            ..Default::default()
        }))
    }

    #[zbus(property)]
    fn available_cursor_modes(&self) -> u32 {
        CURSOR_MODE_EMBEDDED
    }

    #[zbus(property)]
    fn available_source_types(&self) -> u32 {
        SOURCE_TYPE_MONITOR
    }

    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        5
    }
}
