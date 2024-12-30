use drm::control::{
    self, atomic, connector, crtc, property, AtomicCommitFlags, Device as ControlDevice,
};
use drm::Device;
use gbm::Format as DrmFourcc;
use scape_shared::MainMessage;
use tracing::info;

use crate::{Gpu, RendererState};

impl Device for Gpu {}
impl ControlDevice for Gpu {}

impl RendererState {
    pub(crate) fn test_drm(&mut self) {
        let gpu = &self.gpus.values().next().unwrap();

        gpu.set_client_capability(drm::ClientCapability::UniversalPlanes, true)
            .expect("Unable to request UniversalPlanes capability");
        gpu.set_client_capability(drm::ClientCapability::Atomic, true)
            .expect("Unable to request Atomic capability");

        // Load the information.
        let res = gpu
            .resource_handles()
            .expect("Could not load normal resource ids.");
        let coninfo: Vec<connector::Info> = res
            .connectors()
            .iter()
            .flat_map(|con| gpu.get_connector(*con, true))
            .collect();
        let crtcinfo: Vec<crtc::Info> = res
            .crtcs()
            .iter()
            .flat_map(|crtc| gpu.get_crtc(*crtc))
            .collect();

        for crtc in &crtcinfo {
            info!("CRTC: {:?}", crtc);
        }

        // Filter each connector until we find one that's connected.
        let con = coninfo
            .iter()
            .find(|&i| i.state() == connector::State::Connected)
            .expect("No connected connectors");

        let &mode = con.modes().first().expect("No modes found on connector");

        let (disp_width, disp_height) = mode.size();

        // Find a crtc and FB
        let crtc = crtcinfo.first().expect("No crtcs found");

        let fmt = DrmFourcc::Xrgb8888;

        let mut db = gpu
            .create_dumb_buffer((disp_width.into(), disp_height.into()), fmt, 32)
            .expect("Could not create dumb buffer");

        {
            let mut map = gpu
                .map_dumb_buffer(&mut db)
                .expect("Could not map dumbbuffer");
            for b in map.as_mut() {
                *b = 128;
            }
        }

        let fb = gpu
            .add_framebuffer(&db, 24, 32)
            .expect("Could not create FB");

        let planes = gpu.plane_handles().expect("Could not list planes");
        let (better_planes, compatible_planes): (
            Vec<control::plane::Handle>,
            Vec<control::plane::Handle>,
        ) = planes
            .iter()
            .filter(|&&plane| {
                gpu.get_plane(plane)
                    .map(|plane_info| {
                        let compatible_crtcs = res.filter_crtcs(plane_info.possible_crtcs());
                        compatible_crtcs.contains(&crtc.handle())
                    })
                    .unwrap_or(false)
            })
            .partition(|&&plane| {
                if let Ok(props) = gpu.get_properties(plane) {
                    for (&id, &val) in props.iter() {
                        if let Ok(info) = gpu.get_property(id) {
                            if info.name().to_str().map(|x| x == "type").unwrap_or(false) {
                                return val == (drm::control::PlaneType::Primary as u32).into();
                            }
                        }
                    }
                }
                false
            });
        let plane = *better_planes.first().unwrap_or(&compatible_planes[0]);

        tracing::error!("{:#?}", mode);
        tracing::error!("{:#?}", fb);
        tracing::error!("{:#?}", db);
        tracing::error!("{:#?}", plane);

        let card = gpu;

        let con_props = card
            .get_properties(con.handle())
            .expect("Could not get props of connector")
            .as_hashmap(*card)
            .expect("Could not get a prop from connector");
        let crtc_props = card
            .get_properties(crtc.handle())
            .expect("Could not get props of crtc")
            .as_hashmap(*card)
            .expect("Could not get a prop from crtc");
        let plane_props = card
            .get_properties(plane)
            .expect("Could not get props of plane")
            .as_hashmap(*card)
            .expect("Could not get a prop from plane");

        tracing::error!("props: {:#?}", con_props.keys());
        tracing::error!("crtc: {:#?}", crtc_props.keys());
        tracing::error!("plane: {:#?}", plane_props.keys());
        let mut atomic_req = atomic::AtomicModeReq::new();
        atomic_req.add_property(
            con.handle(),
            con_props["CRTC_ID"].handle(),
            property::Value::CRTC(Some(crtc.handle())),
        );
        let blob = card
            .create_property_blob(&mode)
            .expect("Failed to create blob");
        atomic_req.add_property(crtc.handle(), crtc_props["MODE_ID"].handle(), blob);
        atomic_req.add_property(
            crtc.handle(),
            crtc_props["ACTIVE"].handle(),
            property::Value::Boolean(true),
        );
        atomic_req.add_property(
            plane,
            plane_props["FB_ID"].handle(),
            property::Value::Framebuffer(Some(fb)),
        );
        atomic_req.add_property(
            plane,
            plane_props["CRTC_ID"].handle(),
            property::Value::CRTC(Some(crtc.handle())),
        );
        atomic_req.add_property(
            plane,
            plane_props["SRC_X"].handle(),
            property::Value::UnsignedRange(0),
        );
        atomic_req.add_property(
            plane,
            plane_props["SRC_Y"].handle(),
            property::Value::UnsignedRange(0),
        );
        atomic_req.add_property(
            plane,
            plane_props["SRC_W"].handle(),
            property::Value::UnsignedRange((mode.size().0 as u64) << 16),
        );
        atomic_req.add_property(
            plane,
            plane_props["SRC_H"].handle(),
            property::Value::UnsignedRange((mode.size().1 as u64) << 16),
        );
        atomic_req.add_property(
            plane,
            plane_props["CRTC_X"].handle(),
            property::Value::SignedRange(0),
        );
        atomic_req.add_property(
            plane,
            plane_props["CRTC_Y"].handle(),
            property::Value::SignedRange(0),
        );
        atomic_req.add_property(
            plane,
            plane_props["CRTC_W"].handle(),
            property::Value::UnsignedRange(mode.size().0 as u64),
        );
        atomic_req.add_property(
            plane,
            plane_props["CRTC_H"].handle(),
            property::Value::UnsignedRange(mode.size().1 as u64),
        );

        // Set the crtc
        // On many setups, this requires root access.
        card.atomic_commit(AtomicCommitFlags::ALLOW_MODESET, atomic_req)
            .expect("Failed to set mode");

        let five_seconds = ::std::time::Duration::from_millis(5000);
        ::std::thread::sleep(five_seconds);

        self.comms.main(MainMessage::Shutdown);
    }
}
