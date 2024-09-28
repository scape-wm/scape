use super::screen_cast::SessionSourceOptions;
use zbus::{interface, zvariant::OwnedObjectPath, SignalContext};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionType {
    ScreenCast,
    Remote,
}

#[derive(Debug, Clone)]
pub struct Session {
    pub session_type: SessionType,
    pub handle_path: OwnedObjectPath,
    pub source_options: SessionSourceOptions,
}

impl Session {
    pub fn new(
        path: impl Into<OwnedObjectPath>,
        session_type: SessionType,
        source_options: SessionSourceOptions,
    ) -> Self {
        Self {
            session_type,
            handle_path: path.into(),
            source_options,
        }
    }
}

#[interface(name = "org.freedesktop.impl.portal.Session")]
impl Session {
    async fn close(
        &self,
        #[zbus(signal_context)] cxts: SignalContext<'_>,
        #[zbus(object_server)] server: &zbus::ObjectServer,
    ) -> zbus::fdo::Result<()> {
        server
            .remove::<Self, &OwnedObjectPath>(&self.handle_path)
            .await?;
        // TODO: stop screen casting
        Self::closed(&cxts).await?;
        Ok(())
    }

    #[zbus(signal)]
    async fn closed(signal_ctxt: &SignalContext<'_>) -> zbus::Result<()>;

    #[zbus(property, name = "version")]
    fn version(&self) -> u32 {
        2
    }
}
