use calloop::LoopHandle;
use drm::node::DrmNode;
use smithay::backend::udev::{UdevBackend, UdevEvent};

use crate::RendererState;

pub(crate) fn init_udev_device_listener_for_seat(
    seat_name: String,
    loop_handle: LoopHandle<'static, RendererState>,
) -> anyhow::Result<()> {
    let udev_backend = UdevBackend::new(seat_name)?;
    loop_handle
        .insert_source(udev_backend, |event, _, state| {
            UdevEvent::Added { device_id, path } => {
                if let Err(err) = DrmNode::from_dev_id(device_id)
                    .and_then(|node| state.add_device(node, &path))
                {
                    error!("Skipping device {device_id}: {err}");
                }
            }
            UdevEvent::Changed { device_id } => {
                if let Ok(node) = DrmNode::from_dev_id(device_id) {
                    device_changed(state, node)
                }
            }
            UdevEvent::Removed { device_id } => {
                if let Ok(node) = DrmNode::from_dev_id(device_id) {
                    device_removed(state, node)
                }
            }
        })
        .map_err(|e| anyhow::anyhow!("Unable to insert udev backend into event loop: {}", e))?;

    Ok(())
}
