use std::{
    collections::{HashMap, VecDeque},
    fmt::Debug,
    hash::{Hash, Hasher},
    time::Instant,
};

use atm0s_sdn::features::{
    dht_kv::{self, Key, MapControl},
    pubsub::{self, ChannelControl, ChannelId, Feedback},
    FeaturesControl, FeaturesEvent,
};
use media_server_protocol::{
    endpoint::{PeerId, TrackMeta, TrackName},
    media::{MediaPacket, TrackInfo},
};
use sans_io_runtime::Task;

use crate::transport::{LocalTrackId, RemoteTrackId};

use super::{ClusterEndpointControl, ClusterEndpointEvent, ClusterLocalTrackControl, ClusterLocalTrackEvent, ClusterRemoteTrackControl, ClusterRemoteTrackEvent, ClusterRoomHash, Output};

#[derive(num_enum::TryFromPrimitive)]
#[repr(u8)]
pub enum FeedbackKind {
    Bitrate = 0,
    KeyFrameRequest = 1,
}

pub enum Input<Owner> {
    Sdn(FeaturesEvent),
    Endpoint(Owner, ClusterEndpointControl),
}

pub struct ClusterRoom<Owner> {
    room: ClusterRoomHash,
    room_map: dht_kv::Map,
    peers: HashMap<Owner, PeerId>,
    /// track from this node
    local_tracks: HashMap<(Owner, RemoteTrackId), (PeerId, TrackName, dht_kv::Key, pubsub::ChannelId)>,
    local_tracks_source: HashMap<pubsub::ChannelId, (Owner, RemoteTrackId)>,
    /// track info from SDN
    remote_tracks: HashMap<dht_kv::Key, (PeerId, TrackName, TrackMeta)>,
    subscribers: HashMap<ChannelId, Vec<(Owner, LocalTrackId)>>,
    subscribers_source: HashMap<(Owner, LocalTrackId), (ChannelId, PeerId, TrackName)>,
    queue: VecDeque<Output<Owner>>,
}

impl<Owner: Debug + Copy + Clone + Hash + Eq> Task<Input<Owner>, Output<Owner>> for ClusterRoom<Owner> {
    fn on_tick(&mut self, now: Instant) -> Option<Output<Owner>> {
        None
    }

    fn on_event(&mut self, now: Instant, input: Input<Owner>) -> Option<Output<Owner>> {
        match input {
            Input::Endpoint(owner, control) => self.on_endpoint_control(now, owner, control),
            Input::Sdn(event) => self.on_sdn_event(now, event),
        }
    }

    fn pop_output(&mut self, now: Instant) -> Option<Output<Owner>> {
        self.queue.pop_front()
    }

    fn shutdown(&mut self, now: Instant) -> Option<Output<Owner>> {
        None
    }
}

impl<Owner: Debug + Copy + Clone + Hash + Eq> ClusterRoom<Owner> {
    pub fn new(room: ClusterRoomHash) -> Self {
        Self {
            room_map: room.0.into(),
            room,
            peers: HashMap::new(),
            local_tracks: HashMap::new(),
            local_tracks_source: HashMap::new(),
            remote_tracks: HashMap::new(),
            subscribers: HashMap::new(),
            subscribers_source: HashMap::new(),
            queue: VecDeque::new(),
        }
    }

    fn on_sdn_event(&mut self, now: Instant, event: FeaturesEvent) -> Option<Output<Owner>> {
        match event {
            FeaturesEvent::DhtKv(event) => match event {
                dht_kv::Event::MapEvent(map, event) => {
                    if self.room_map == map {
                        match event {
                            dht_kv::MapEvent::OnSet(track_key, _source, data) => self.on_room_kv_event(track_key, Some(data)),
                            dht_kv::MapEvent::OnDel(track_key, _source) => self.on_room_kv_event(track_key, None),
                            dht_kv::MapEvent::OnRelaySelected(_) => None,
                        }
                    } else {
                        None
                    }
                }
                dht_kv::Event::MapGetRes(_, _) => None,
            },
            FeaturesEvent::PubSub(pubsub::Event(channel, event)) => match event {
                pubsub::ChannelEvent::RouteChanged(_) => self.on_channel_source_changed(channel),
                pubsub::ChannelEvent::SourceData(_, data) => self.on_channel_pkt(channel, data),
                pubsub::ChannelEvent::FeedbackData(fb) => self.on_channel_feedback(channel, fb),
            },
            _ => None,
        }
    }

    fn on_endpoint_control(&mut self, now: Instant, owner: Owner, control: ClusterEndpointControl) -> Option<Output<Owner>> {
        match control {
            ClusterEndpointControl::Join(peer) => {
                log::info!("[ClusterRoom {}] join peer ({peer})", self.room);
                self.peers.insert(owner.clone(), peer);
                if self.peers.len() == 1 {
                    log::info!("[ClusterRoom {}] first peer join => subscribe room map", self.room);
                    Some(Output::Sdn(self.room, FeaturesControl::DhtKv(dht_kv::Control::MapCmd(self.room_map, MapControl::Sub))))
                } else {
                    log::info!("[ClusterRoom {}] next peer join => restore {} remote tracks", self.room, self.remote_tracks.len());
                    for (_track_key, (peer, name, meta)) in &self.remote_tracks {
                        self.queue
                            .push_back(Output::Endpoint(vec![owner.clone()], ClusterEndpointEvent::TrackStarted(peer.clone(), name.clone(), meta.clone())));
                    }

                    self.queue.pop_front()
                }
            }
            ClusterEndpointControl::Leave => {
                let peer = self.peers.remove(&owner).expect("Should have owner");
                log::info!("[ClusterRoom {}] leave peer ({peer})", self.room);
                if self.peers.is_empty() {
                    log::info!("[ClusterRoom {}] last peer leave => unsubscribe room map", self.room);
                    Some(Output::Sdn(self.room, FeaturesControl::DhtKv(dht_kv::Control::MapCmd(self.room_map, MapControl::Unsub))))
                } else {
                    None
                }
            }
            ClusterEndpointControl::RemoteTrack(track, control) => self.control_remote_track(now, owner, track, control),
            ClusterEndpointControl::LocalTrack(track, control) => self.control_local_track(now, owner, track, control),
        }
    }
}

impl<Owner: Debug + Clone + Copy + Hash + Eq> ClusterRoom<Owner> {
    fn control_remote_track(&mut self, now: Instant, owner: Owner, track: RemoteTrackId, control: ClusterRemoteTrackControl) -> Option<Output<Owner>> {
        match control {
            ClusterRemoteTrackControl::Started(name, meta) => {
                let peer = self.peers.get(&owner)?;
                log::info!("[ClusterRoom {}] peer ({peer} started track {name})", self.room);
                let map_key: Key = self.track_key(&peer, &name);
                let channel_id = (*map_key).into();
                self.local_tracks.insert((owner, track), (peer.clone(), name.clone(), map_key, channel_id));
                self.local_tracks_source.insert(channel_id, (owner, track));
                let info = TrackInfo {
                    peer: peer.clone(),
                    track: name,
                    meta,
                };
                self.queue
                    .push_back(Output::Sdn(self.room, FeaturesControl::PubSub(pubsub::Control(channel_id, ChannelControl::PubStart))));
                Some(Output::Sdn(
                    self.room,
                    FeaturesControl::DhtKv(dht_kv::Control::MapCmd((*self.room.as_ref()).into(), MapControl::Set(map_key, info.serialize()))),
                ))
            }
            ClusterRemoteTrackControl::Media(media) => {
                let (_peer, _name, _key, channel_id) = self.local_tracks.get(&(owner, track))?;
                let data = media.serialize();
                Some(Output::Sdn(self.room, FeaturesControl::PubSub(pubsub::Control(*channel_id, ChannelControl::PubData(data)))))
            }
            ClusterRemoteTrackControl::Ended => {
                let (peer, name, map_key, channel_id) = self.local_tracks.remove(&(owner, track))?;
                log::info!("[ClusterRoom {}] peer ({peer} stopped track {name})", self.room);
                self.queue
                    .push_back(Output::Sdn(self.room, FeaturesControl::PubSub(pubsub::Control(channel_id, ChannelControl::PubStop))));
                Some(Output::Sdn(
                    self.room,
                    FeaturesControl::DhtKv(dht_kv::Control::MapCmd((*self.room.as_ref()).into(), MapControl::Del(map_key))),
                ))
            }
        }
    }

    fn control_local_track(&mut self, now: Instant, owner: Owner, track_id: LocalTrackId, control: ClusterLocalTrackControl) -> Option<Output<Owner>> {
        match control {
            ClusterLocalTrackControl::Subscribe(peer, track) => {
                let current_peer = self.peers.get(&owner)?;
                let channel_id: ChannelId = self.track_key(&peer, &track);
                log::info!(
                    "[ClusterRoom {}] peer ({current_peer} subscribe peer {peer} track {track}) in track {track_id}, channel: {channel_id}",
                    self.room
                );
                self.subscribers_source.insert((owner, track_id), (channel_id, peer, track));
                let subscribers = self.subscribers.entry(channel_id).or_insert(Default::default());
                subscribers.push((owner, track_id));
                if subscribers.len() == 1 {
                    log::info!("[ClusterRoom {}] first subscriber => Sub channel {channel_id}", self.room);
                    Some(Output::Sdn(self.room, FeaturesControl::PubSub(pubsub::Control(channel_id, ChannelControl::SubAuto))))
                } else {
                    None
                }
            }
            ClusterLocalTrackControl::RequestKeyFrame => {
                let (channel_id, peer, track) = self.subscribers_source.get(&(owner, track_id))?;
                log::info!("[ClusterRoom {}] request key-frame {channel_id} {peer} {track}", self.room);
                Some(Output::Sdn(
                    self.room,
                    FeaturesControl::PubSub(pubsub::Control(
                        *channel_id,
                        ChannelControl::FeedbackAuto(Feedback::simple(FeedbackKind::KeyFrameRequest as u8, 1, 100, 200)),
                    )),
                ))
            }
            ClusterLocalTrackControl::Unsubscribe => {
                let current_peer = self.peers.get(&owner)?;
                let (channel_id, peer, track) = self.subscribers_source.get(&(owner, track_id))?;
                let subscribers = self.subscribers.get_mut(channel_id)?;
                let (index, _) = subscribers.iter().enumerate().find(|e| e.1.eq(&(owner, track_id)))?;
                subscribers.swap_remove(index);
                log::info!(
                    "[ClusterRoom {}] peer ({current_peer} unsubscribe with track {track_id} from source {peer} {track}, channel {channel_id}",
                    self.room
                );
                if subscribers.is_empty() {
                    log::info!("[ClusterRoom {}] last unsubscriber => Unsub channel {channel_id}", self.room);
                    Some(Output::Sdn(self.room, FeaturesControl::PubSub(pubsub::Control(*channel_id, ChannelControl::UnsubAuto))))
                } else {
                    None
                }
            }
        }
    }
}

impl<Owner: Debug + Clone + Hash + Eq> ClusterRoom<Owner> {
    fn on_room_kv_event(&mut self, track: dht_kv::Key, data: Option<Vec<u8>>) -> Option<Output<Owner>> {
        let info = if let Some(data) = data {
            Some(TrackInfo::deserialize(&data)?)
        } else {
            None
        };

        let peers = self.peers.keys().cloned().collect::<Vec<_>>();
        if let Some(info) = info {
            log::info!("[ClusterRoom {}] cluster: peer ({}) started track {}) => fire event to {:?}", self.room, info.peer, info.track, peers);
            self.remote_tracks.insert(track, (info.peer.clone(), info.track.clone(), info.meta.clone()));
            Some(Output::Endpoint(peers, ClusterEndpointEvent::TrackStarted(info.peer, info.track, info.meta)))
        } else {
            let (peer, name, _meta) = self.remote_tracks.remove(&track)?;
            log::info!("[ClusterRoom {}] cluster: peer ({}) stopped track {}) => fire event to {:?}", self.room, peer, name, peers);
            Some(Output::Endpoint(peers, ClusterEndpointEvent::TrackStoped(peer, name)))
        }
    }

    fn on_channel_source_changed(&mut self, channel: ChannelId) -> Option<Output<Owner>> {
        let subscribers = self.subscribers.get(&channel)?;
        log::info!("[ClusterRoom {}] cluster: channel {channel} source changed => fire event to {:?}", self.room, subscribers);
        for (owner, track) in subscribers {
            self.queue
                .push_back(Output::Endpoint(vec![owner.clone()], ClusterEndpointEvent::LocalTrack(*track, ClusterLocalTrackEvent::SourceChanged)))
        }
        self.queue.pop_front()
    }

    fn on_channel_pkt(&mut self, channel: ChannelId, data: Vec<u8>) -> Option<Output<Owner>> {
        let pkt = MediaPacket::deserialize(&data)?;
        let subscribers = self.subscribers.get(&channel)?;
        for (owner, track) in subscribers {
            self.queue.push_back(Output::Endpoint(
                vec![owner.clone()],
                ClusterEndpointEvent::LocalTrack(*track, ClusterLocalTrackEvent::Media(pkt.clone())),
            ))
        }
        self.queue.pop_front()
    }

    fn on_channel_feedback(&mut self, channel: ChannelId, fb: Feedback) -> Option<Output<Owner>> {
        let fb_kind = FeedbackKind::try_from(fb.kind).ok()?;
        let (owner, track_id) = self.local_tracks_source.get(&channel)?;
        match fb_kind {
            FeedbackKind::Bitrate => todo!(),
            FeedbackKind::KeyFrameRequest => Some(Output::Endpoint(
                vec![owner.clone()],
                ClusterEndpointEvent::RemoteTrack(*track_id, ClusterRemoteTrackEvent::RequestKeyFrame),
            )),
        }
    }

    fn track_key<T: From<u64>>(&self, peer: &PeerId, track: &TrackName) -> T {
        let mut h = std::hash::DefaultHasher::new();
        self.room.as_ref().hash(&mut h);
        peer.as_ref().hash(&mut h);
        track.as_ref().hash(&mut h);
        h.finish().into()
    }
}
