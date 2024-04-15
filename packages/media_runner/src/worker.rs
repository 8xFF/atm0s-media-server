use std::time::Instant;

use atm0s_sdn::{services::visualization, SdnWorker, SdnWorkerCfg};
use media_server_core::cluster::MediaCluster;
use transport_webrtc::MediaWorkerWebrtc;

//for sdn
pub type SC = visualization::Control;
pub type SE = visualization::Event;
pub type TC = ();
pub type TW = ();

pub enum Input {
    ExtRpc(u64, media_server_protocol::transport::RpcReq),
}

pub enum Output {
    ExtRpc(u64, media_server_protocol::transport::RpcRes),
}

pub struct MediaServerWorker {
    sdn_worker: SdnWorker<SC, SE, TC, TW>,
    media_network: MediaCluster,
    media_webrtc: MediaWorkerWebrtc,
}

impl MediaServerWorker {
    pub fn new(sdn: SdnWorkerCfg<SC, SE, TC, TW>) -> Self {
        Self {
            sdn_worker: SdnWorker::new(sdn),
            media_network: MediaCluster::default(),
            media_webrtc: MediaWorkerWebrtc::new(),
        }
    }

    pub fn on_tick(&mut self, now: Instant) -> Option<Output> {
        todo!()
    }

    pub fn on_event(&mut self, now: Instant, input: Input) -> Option<Output> {
        todo!()
    }

    pub fn pop_output(&mut self, now: Instant) -> Option<Output> {
        todo!()
    }

    pub fn shutdown(&mut self, now: Instant) -> Option<Output> {
        todo!()
    }
}
