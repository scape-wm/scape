use zbus::{interface, zvariant::OwnedObjectPath};

pub struct Request {
    pub handle_path: OwnedObjectPath,
}

#[interface(name = "org.freedesktop.impl.portal.Request")]
impl Request {
    async fn close(
        &self,
        #[zbus(object_server)] server: &zbus::ObjectServer,
    ) -> zbus::fdo::Result<()> {
        server
            .remove::<Self, &OwnedObjectPath>(&self.handle_path)
            .await?;
        Ok(())
    }
}
