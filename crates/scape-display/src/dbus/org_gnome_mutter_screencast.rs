//! Implementation of the org.gnome.Mutter.ScreenCast D-Bus interface as defined in
//! https://gitlab.gnome.org/GNOME/gnome-shell/-/blob/main/data/dbus-interfaces/org.gnome.Mutter.ScreenCast.xml

use std::{
    collections::HashMap,
    sync::atomic::{AtomicUsize, Ordering},
};

use tracing::{error, info};
use zbus::{
    connection, fdo, interface,
    object_server::SignalContext,
    zvariant::{DeserializeDict, OwnedObjectPath, SerializeDict, Type, Value},
    Connection, ObjectServer,
};

const VERSION: i32 = 4;
const SCREENCAST_SESSION_BASE_PATH: &str = "/org/gnome/Mutter/ScreenCast/Session";

#[derive(Default)]
struct ScreenCast {
    session_id_counter: AtomicUsize,
}

#[interface(name = "org.gnome.Mutter.ScreenCast")]
impl ScreenCast {
    async fn create_session(
        &self,
        #[zbus(object_server)] object_server: &ObjectServer,
        #[allow(unused)] properties: HashMap<&str, Value<'_>>,
    ) -> fdo::Result<OwnedObjectPath> {
        info!("Creating screencast session");

        let session_id = self.session_id_counter.fetch_add(1, Ordering::SeqCst);
        let path = format!("{SCREENCAST_SESSION_BASE_PATH}/session_{session_id}");
        let path = match OwnedObjectPath::try_from(path) {
            Ok(path) => path,
            Err(err) => {
                error!(?err, "Unable to create session path");
                return Err(fdo::Error::Failed(format!(
                    "Unable to create session: {err}",
                )));
            }
        };

        let session = Session::new(session_id);
        match object_server.at(&path, session.clone()).await {
            Ok(true) => {
                info!(%path, "Created screencat session");
                Ok(path)
            }
            Ok(false) => Err(fdo::Error::Failed(
                "Session path already exists".to_string(),
            )),
            Err(err) => Err(fdo::Error::Failed(format!(
                "Error creating session object: {err}"
            ))),
        }
    }

    #[zbus(property)]
    async fn version(&self) -> i32 {
        VERSION
    }
}

#[derive(Clone, Debug)]
pub struct Session {
    id: usize,
}

impl Session {
    fn new(id: usize) -> Self {
        Self { id }
    }
}

#[interface(name = "org.gnome.Mutter.ScreenCast.Session")]
impl Session {
    async fn start(&self) {}

    pub async fn stop(
        &self,
        #[zbus(object_server)] server: &ObjectServer,
        #[zbus(signal_context)] ctxt: SignalContext<'_>,
    ) {
    }

    async fn record_monitor(
        &mut self,
        #[zbus(object_server)] object_server: &ObjectServer,
        connector: &str,
        properties: RecordMonitorProperties,
    ) -> fdo::Result<OwnedObjectPath> {
        Ok(OwnedObjectPath::try_from("/test").unwrap())
    }

    async fn record_window(
        &mut self,
        #[zbus(object_server)] server: &ObjectServer,
        properties: RecordWindowProperties,
    ) -> fdo::Result<OwnedObjectPath> {
        Ok(OwnedObjectPath::try_from("/test").unwrap())
    }

    #[zbus(signal)]
    async fn closed(ctxt: &SignalContext<'_>) -> zbus::Result<()>;
}

#[derive(Debug, DeserializeDict, Type)]
#[zvariant(signature = "dict")]
struct RecordMonitorProperties {
    #[zvariant(rename = "cursor-mode")]
    cursor_mode: Option<u32>,
    #[zvariant(rename = "is-recording")]
    is_recording: Option<bool>,
}

#[derive(Debug, DeserializeDict, Type)]
#[zvariant(signature = "dict")]
struct RecordWindowProperties {
    #[zvariant(rename = "window-id")]
    window_id: u64,
    #[zvariant(rename = "cursor-mode")]
    cursor_mode: Option<u32>,
    #[zvariant(rename = "is-recording")]
    is_recording: Option<bool>,
}

#[derive(Clone)]
pub struct Stream {}

#[interface(name = "org.gnome.Mutter.ScreenCast.Stream")]
impl Stream {
    #[zbus(signal)]
    pub async fn pipe_wire_stream_added(ctxt: &SignalContext<'_>, node_id: u32)
        -> zbus::Result<()>;

    #[zbus(property)]
    async fn parameters(&self) -> StreamParameters {
        StreamParameters {
            position: (0, 0),
            size: (1, 1),
        }
    }
}

#[derive(Debug, SerializeDict, Type, Value)]
#[zvariant(signature = "dict")]
struct StreamParameters {
    position: (i32, i32),
    size: (i32, i32),
}

pub async fn start() -> anyhow::Result<Connection> {
    Ok(connection::Builder::session()?
        .name("org.gnome.Mutter.ScreenCast")?
        .serve_at("/org/gnome/Mutter/ScreenCast", ScreenCast::default())?
        .build()
        .await?)
}
