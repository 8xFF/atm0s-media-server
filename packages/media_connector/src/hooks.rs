pub mod events;
pub mod storage;
pub mod worker;

use std::{io::Error, sync::Arc};

use async_trait::async_trait;
use atm0s_sdn::NodeId;
use media_server_protocol::protobuf::cluster_connector::{connector_request, peer_event};
use media_server_utils::now_ms;
use storage::HookStorage;
use uuid::{NoContext, Timestamp};
use worker::HookWorker;

use crate::hooks::events::HookEvent;

#[async_trait]
pub trait HookPublisher {
    async fn publish(&self, event: HookEvent) -> Option<Error>;
}

pub struct HookControllerCfg {
    pub worker_num: u16,
    pub job_num: u16,
}

pub struct ConnectorHookController {
    cfg: HookControllerCfg,
    workers: Vec<HookWorker>,
    storage: Arc<dyn HookStorage>,
}

impl ConnectorHookController {
    pub fn new(publisher: Option<Arc<dyn HookPublisher>>, cfg: HookControllerCfg) -> Self {
        let mut workers = Vec::new();
        for _ in 0..cfg.worker_num {
            workers.push(HookWorker::new(publisher.clone()));
        }
        Self {
            cfg,
            workers,
            storage: Arc::new(storage::InMemoryHookStorage::default()),
        }
    }

    pub fn on_event(&mut self, from: NodeId, ts: u64, event: connector_request::Request) -> Option<()> {
        match event {
            connector_request::Request::Peer(event) => {
                if let Some(ev) = event.event {
                    self.on_peer_event(from, ts, event.session_id, ev);
                }
            }
            connector_request::Request::Record(_) => {}
        }
        Some(())
    }

    fn on_peer_event(&mut self, from: NodeId, ts: u64, session: u64, ev: peer_event::Event) -> Option<()> {
        let uuid = uuid::Uuid::new_v7(Timestamp::from_unix(NoContext, ts, 0)).to_string();
        let hook_data: Option<HookEvent> = match ev {
            peer_event::Event::RouteBegin(_params) => None,
            peer_event::Event::RouteSuccess(_params) => None,
            peer_event::Event::RouteError(_params) => None,
            peer_event::Event::Connecting(params) => Some(HookEvent::Session {
                uuid,
                node: from,
                ts,
                session,
                state: crate::hooks::events::SessionState::Connecting,
                remote_ip: Some(params.remote_ip),
                after_ms: None,
                duration: None,
                reason: None,
                error: None,
            }),
            peer_event::Event::Connected(params) => Some(HookEvent::Session {
                uuid,
                node: from,
                ts,
                session,
                state: crate::hooks::events::SessionState::Connected,
                remote_ip: Some(params.remote_ip),
                after_ms: Some(params.after_ms),
                duration: None,
                reason: None,
                error: None,
            }),
            peer_event::Event::ConnectError(params) => Some(HookEvent::Session {
                uuid,
                node: from,
                ts,
                session,
                state: crate::hooks::events::SessionState::ConnectError,
                remote_ip: None,
                after_ms: Some(params.after_ms),
                duration: None,
                reason: None,
                error: Some(params.error),
            }),
            peer_event::Event::Reconnect(params) => Some(HookEvent::Session {
                uuid,
                node: from,
                ts,
                session,
                state: crate::hooks::events::SessionState::Reconnect,
                remote_ip: Some(params.remote_ip),
                after_ms: None,
                duration: None,
                reason: None,
                error: None,
            }),
            peer_event::Event::Reconnected(params) => Some(HookEvent::Session {
                uuid,
                node: from,
                ts,
                session,
                state: crate::hooks::events::SessionState::Reconnected,
                remote_ip: Some(params.remote_ip),
                after_ms: Some(params.after_ms),
                duration: None,
                reason: None,
                error: None,
            }),
            peer_event::Event::Disconnected(params) => Some(HookEvent::Session {
                uuid,
                node: from,
                ts,
                session,
                state: crate::hooks::events::SessionState::Disconnected,
                remote_ip: None,
                after_ms: None,
                duration: Some(params.duration_ms),
                reason: Some(params.reason),
                error: None,
            }),
            peer_event::Event::Join(params) => Some(HookEvent::Peer {
                uuid,
                node: from,
                ts,
                session,
                room: params.room,
                peer: params.peer,
                event: crate::hooks::events::PeerEvent::Joined,
            }),
            peer_event::Event::Leave(params) => Some(HookEvent::Peer {
                uuid,
                node: from,
                ts,
                session,
                room: params.room,
                peer: params.peer,
                event: crate::hooks::events::PeerEvent::Leaved,
            }),
            peer_event::Event::RemoteTrackStarted(params) => Some(HookEvent::RemoteTrack {
                uuid,
                node: from,
                ts,
                session,
                track: params.track,
                kind: params.kind,
                event: crate::hooks::events::RemoteTrackEvent::Started,
            }),
            peer_event::Event::RemoteTrackEnded(params) => Some(HookEvent::RemoteTrack {
                uuid,
                node: from,
                ts,
                session,
                track: params.track,
                kind: params.kind,
                event: crate::hooks::events::RemoteTrackEvent::Ended,
            }),
            peer_event::Event::LocalTrack(params) => Some(HookEvent::LocalTrack {
                uuid,
                node: from,
                ts,
                session,
                track: params.track,
                event: crate::hooks::events::LocalTrackEvent::LocalTrack,
                kind: Some(params.kind),
                remote_peer: None,
                remote_track: None,
            }),
            peer_event::Event::LocalTrackAttach(params) => Some(HookEvent::LocalTrack {
                uuid,
                node: from,
                ts,
                session,
                track: params.track,
                event: crate::hooks::events::LocalTrackEvent::Attached,
                kind: None,
                remote_peer: Some(params.remote_peer),
                remote_track: Some(params.remote_track),
            }),
            peer_event::Event::LocalTrackDetach(params) => Some(HookEvent::LocalTrack {
                uuid,
                node: from,
                ts,
                session,
                track: params.track,
                event: crate::hooks::events::LocalTrackEvent::Detached,
                kind: None,
                remote_peer: Some(params.remote_peer),
                remote_track: Some(params.remote_track),
            }),
            peer_event::Event::Stats(_params) => None,
        };
        if let Some(hook_data) = hook_data {
            log::error!("hook_data: {:?}", hook_data);
            self.storage.push_back(hook_data);
        }
        Some(())
    }

    pub async fn on_tick(&mut self) {
        // log::error!("on_tick: on worker ticks");
        for worker in self.workers.iter_mut() {
            worker.on_tick().await;
        }

        let jobs = self.storage.jobs(self.cfg.job_num as i16);
        for job in jobs.iter() {
            // log::error!("job: {:?}", job.payload);
            let path = job.payload.session() % (self.cfg.worker_num as u64);
            self.workers[path as usize].push(job.clone());
        }

        self.storage.clean_timeout_event(now_ms());
    }
}
