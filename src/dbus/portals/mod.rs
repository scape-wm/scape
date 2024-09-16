use screen_cast::ScreenCast;
use std::collections::HashMap;
use zbus::export::serde;
use zbus::{connection, zvariant, Connection};

pub mod request;
pub mod screen_cast;
pub mod session;

const PORTAL_RESPONSE_SUCCESS: u32 = 0;
const PORTAL_RESPONSE_CANCELLED: u32 = 1;
const PORTAL_RESPONSE_OTHER: u32 = 2;

#[derive(zvariant::Type)]
#[zvariant(signature = "(ua{sv})")]
enum PortalResponse<T: zvariant::Type + serde::Serialize> {
    Success(T),
    Cancelled,
    Other,
}

impl<T: zvariant::Type + serde::Serialize> serde::Serialize for PortalResponse<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Success(res) => (PORTAL_RESPONSE_SUCCESS, res).serialize(serializer),
            Self::Cancelled => (
                PORTAL_RESPONSE_CANCELLED,
                HashMap::<String, zvariant::Value>::new(),
            )
                .serialize(serializer),
            Self::Other => (
                PORTAL_RESPONSE_OTHER,
                HashMap::<String, zvariant::Value>::new(),
            )
                .serialize(serializer),
        }
    }
}

pub async fn start() -> anyhow::Result<Connection> {
    Ok(connection::Builder::session()?
        .name("org.freedesktop.impl.portal.desktop.scape")?
        .serve_at("/org/freedesktop/portal/desktop", ScreenCast::default())?
        .build()
        .await?)
}
