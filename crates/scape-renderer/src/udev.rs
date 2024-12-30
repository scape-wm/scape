use std::path::Path;

use drm::node::{DrmNode, NodeType};
use scape_shared::InputMessage;
use smithay::backend::udev::{all_gpus, primary_gpu, UdevBackend, UdevEvent};
use tracing::error;

use crate::RendererState;

impl RendererState {
    pub(crate) fn init_udev_device_listener_for_seat(
        &mut self,
        seat_name: &str,
    ) -> anyhow::Result<()> {
        let udev_backend = UdevBackend::new(seat_name)?;

        for (dev_id, path) in udev_backend.device_list() {
            self.add_device(DrmNode::from_dev_id(dev_id)?, path);
        }

        self.loop_handle
            .insert_source(udev_backend, |event, _, state| match event {
                UdevEvent::Added { device_id, path } => {
                    if let Err(err) =
                        DrmNode::from_dev_id(device_id).map(|node| state.add_device(node, &path))
                    {
                        error!("Skipping device {device_id}: {err}");
                    }
                }
                UdevEvent::Changed { device_id } => {
                    if let Ok(node) = DrmNode::from_dev_id(device_id) {
                        state.device_changed(node)
                    }
                }
                UdevEvent::Removed { device_id } => {
                    if let Ok(node) = DrmNode::from_dev_id(device_id) {
                        state.device_removed(node)
                    }
                }
            })
            .map_err(|e| anyhow::anyhow!("Unable to insert udev backend into event loop: {}", e))?;

        self.select_primary_gpu(seat_name)
    }

    pub(crate) fn add_device(&mut self, node: DrmNode, path: &Path) {
        self.known_drm_devices.insert(node);
        self.comms
            .input(InputMessage::OpenFileInSessionForRenderer {
                path: path.to_path_buf(),
            });
    }

    pub(crate) fn device_changed(&mut self, node: DrmNode) {}

    pub(crate) fn device_removed(&mut self, node: DrmNode) {}

    fn select_primary_gpu(&mut self, seat_name: &str) -> anyhow::Result<()> {
        let primary_gpu = primary_gpu(seat_name)?
            .and_then(|x| {
                DrmNode::from_path(x)
                    .ok()?
                    .node_with_type(NodeType::Render)?
                    .ok()
            })
            .or_else(|| {
                all_gpus(seat_name)
                    .ok()?
                    .into_iter()
                    .find_map(|x| DrmNode::from_path(x).ok())
            })
            .ok_or(anyhow::anyhow!("Unable to select primary gpu"))?;

        self.primary_gpu = Some(primary_gpu);
        Ok(())
    }
}
