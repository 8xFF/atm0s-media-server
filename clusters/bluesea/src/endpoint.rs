use std::collections::HashMap;

use async_std::channel::{bounded, Receiver, Sender};
use bytes::Bytes;
use cluster::{
    generate_cluster_track_uuid, ClusterEndpoint, ClusterEndpointError, ClusterEndpointIncomingEvent, ClusterEndpointOutgoingEvent, ClusterLocalTrackIncomingEvent, ClusterLocalTrackOutgoingEvent,
    ClusterRemoteTrackIncomingEvent, ClusterRemoteTrackOutgoingEvent,
};
use futures::{select, FutureExt};
use media_utils::hash_str;
use runner::{ChannelUuid, ConsumerRaw, Feedback, FeedbackType, KeyId, KeySource, KeyValueSdk, KeyVersion, LocalSubId, NodeId, NumberInfo, PublisherRaw, PubsubSdk, SubKeyId, ValueType};
use transport::RequestKeyframeKind;

use crate::{
    server::{NodeBehaviorEvent, NodeHandleEvent},
    types::{from_room_value, to_room_key, to_room_value, TrackData},
};

#[repr(u8)]
enum TrackFeedbackType {
    RequestKeyFrame = 0,
    LimitBitrate = 1,
}

impl TryFrom<u8> for TrackFeedbackType {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(TrackFeedbackType::RequestKeyFrame),
            1 => Ok(TrackFeedbackType::LimitBitrate),
            _ => Err(()),
        }
    }
}

pub struct BlueseaClusterEndpoint {
    room_id: String,
    peer_id: String,
    room_key: u64,
    sub_uuid: u64,
    pubsub_sdk: PubsubSdk<NodeBehaviorEvent, NodeHandleEvent>,
    kv_sdk: KeyValueSdk,
    kv_tx: Sender<(KeyId, SubKeyId, Option<ValueType>, KeyVersion, KeySource)>,
    kv_rx: Receiver<(KeyId, SubKeyId, Option<ValueType>, KeyVersion, KeySource)>,
    data_tx: Sender<(LocalSubId, NodeId, ChannelUuid, Bytes)>,
    data_rx: Receiver<(LocalSubId, NodeId, ChannelUuid, Bytes)>,
    data_fb_tx: Sender<Feedback>,
    data_fb_rx: Receiver<Feedback>,
    consumer_map: HashMap<u64, u16>,
    track_sub_map: HashMap<u16, ConsumerRaw>,
    room_sub: Option<()>,
    peer_sub: HashMap<String, ()>,
    track_pub: HashMap<ChannelUuid, (u16, PublisherRaw<NodeBehaviorEvent, NodeHandleEvent>)>,
    remote_track_cached: HashMap<u64, (String, String)>,
}

impl BlueseaClusterEndpoint {
    pub(crate) fn new(room_id: &str, peer_id: &str, pubsub_sdk: PubsubSdk<NodeBehaviorEvent, NodeHandleEvent>, kv_sdk: KeyValueSdk) -> Self {
        let (kv_tx, kv_rx) = bounded(100);
        let (data_tx, data_rx) = bounded(100);
        let (data_fb_tx, data_fb_rx) = bounded(100);

        Self {
            room_id: room_id.to_string(),
            peer_id: peer_id.to_string(),
            room_key: hash_str(room_id),
            sub_uuid: hash_str(&format!("{}/{}", room_id, peer_id)),
            pubsub_sdk,
            kv_sdk,
            kv_tx,
            kv_rx,
            data_tx,
            data_rx,
            data_fb_tx,
            data_fb_rx,
            track_sub_map: Default::default(),
            consumer_map: Default::default(),
            room_sub: None,
            peer_sub: Default::default(),
            track_pub: Default::default(),
            remote_track_cached: Default::default(),
        }
    }

    fn peer_key(&self, peer_id: &str) -> u64 {
        hash_str(&format!("{}/{}", self.room_id, peer_id))
    }
}

#[async_trait::async_trait]
impl ClusterEndpoint for BlueseaClusterEndpoint {
    fn on_event(&mut self, event: ClusterEndpointOutgoingEvent) -> Result<(), ClusterEndpointError> {
        match event {
            ClusterEndpointOutgoingEvent::SubscribeRoom => {
                if self.peer_sub.is_empty() && self.room_sub.is_none() {
                    self.kv_sdk.hsubscribe_raw(self.room_key, self.sub_uuid, Some(10000), self.kv_tx.clone());
                    self.room_sub = Some(());
                } else {
                    log::warn!("[BlueseaClusterEndpoint] sub room but already exist");
                }
                Ok(())
            }
            ClusterEndpointOutgoingEvent::UnsubscribeRoom => {
                if self.peer_sub.is_empty() && self.room_sub.take().is_some() {
                    self.kv_sdk.hunsubscribe_raw(self.room_key, self.sub_uuid);
                } else {
                    log::warn!("[BlueseaClusterEndpoint] unsub room but not found");
                }
                Ok(())
            }
            ClusterEndpointOutgoingEvent::SubscribePeer(peer_id) => {
                if self.room_sub.is_none() && !self.peer_sub.contains_key(&peer_id) {
                    self.kv_sdk.hsubscribe_raw(self.peer_key(&peer_id), self.sub_uuid, Some(10000), self.kv_tx.clone());
                    self.peer_sub.insert(peer_id, ());
                } else {
                    log::warn!("[BlueseaClusterEndpoint] sub peer but already exist {peer_id}");
                }
                Ok(())
            }
            ClusterEndpointOutgoingEvent::UnsubscribePeer(peer_id) => {
                if self.room_sub.is_none() && self.peer_sub.remove(&peer_id).is_some() {
                    self.kv_sdk.hunsubscribe_raw(self.peer_key(&peer_id), self.sub_uuid);
                } else {
                    log::warn!("[BlueseaClusterEndpoint] unsub peer but not found {peer_id}");
                }
                Ok(())
            }
            ClusterEndpointOutgoingEvent::LocalTrackEvent(track_id, event) => match event {
                ClusterLocalTrackOutgoingEvent::RequestKeyFrame(kind) => {
                    let value = match kind {
                        RequestKeyframeKind::Fir => 1,
                        RequestKeyframeKind::Pli => 2,
                    } as i64;
                    if let Some(consumer) = self.track_sub_map.get(&track_id) {
                        log::debug!("[BlueseaClusterEndpoint] send track feedback RequestKeyFrame {track_id} => {:?}", consumer.uuid());
                        consumer.feedback(
                            TrackFeedbackType::RequestKeyFrame as u8,
                            FeedbackType::Number {
                                window_ms: 200,
                                info: NumberInfo {
                                    count: 1,
                                    sum: value,
                                    max: value,
                                    min: value,
                                },
                            },
                        )
                    } else {
                        log::warn!("[BlueseaClusterEndpoint] send track feedback RequestKeyFrame but track not found {track_id}");
                    }
                    Ok(())
                }
                ClusterLocalTrackOutgoingEvent::LimitBitrate(bitrate) => {
                    if let Some(consumer) = self.track_sub_map.get(&track_id) {
                        log::debug!("[BlueseaClusterEndpoint] send track feedback LimitBitrate({bitrate}) {track_id} => {:?}", consumer.uuid());
                        consumer.feedback(
                            TrackFeedbackType::LimitBitrate as u8,
                            FeedbackType::Number {
                                window_ms: 200,
                                info: NumberInfo {
                                    count: 1,
                                    sum: bitrate as i64,
                                    max: bitrate as i64,
                                    min: bitrate as i64,
                                },
                            },
                        )
                    } else {
                        log::warn!("[BlueseaClusterEndpoint] send track feedback LimitBitrate({bitrate}) but track not found {track_id}");
                    }
                    Ok(())
                }
                ClusterLocalTrackOutgoingEvent::Subscribe(peer_id, track_name) => {
                    let track_uuid = generate_cluster_track_uuid(&self.room_id, &peer_id, &track_name);
                    let consumer = self.pubsub_sdk.create_consumer_raw(track_uuid as ChannelUuid, self.data_tx.clone());
                    log::info!("[BlueseaClusterEndpoint] sub track {peer_id} {track_name} => track_uuid {track_uuid} consumer_id {}", consumer.uuid());
                    self.consumer_map.insert(consumer.uuid(), track_id);
                    self.track_sub_map.insert(track_id, consumer);
                    Ok(())
                }
                ClusterLocalTrackOutgoingEvent::Unsubscribe(peer_id, track_name) => {
                    let track_uuid = generate_cluster_track_uuid(&self.room_id, &peer_id, &track_name);
                    if let Some(consumer) = self.track_sub_map.remove(&track_id) {
                        log::info!("[BlueseaClusterEndpoint] unsub track {peer_id} {track_name} => track_uuid {track_uuid} consumer_id {}", consumer.uuid());
                        self.consumer_map.remove(&consumer.uuid());
                    } else {
                        log::warn!("[BlueseaClusterEndpoint] unsub track but not found {peer_id} {track_name} => track_uuid {track_uuid}");
                    }
                    Ok(())
                }
            },
            ClusterEndpointOutgoingEvent::RemoteTrackEvent(track_id, track_uuid, event) => {
                let channel_uuid = track_uuid as ChannelUuid;
                match event {
                    ClusterRemoteTrackOutgoingEvent::TrackAdded(track_name, track_meta) => {
                        if !self.track_pub.contains_key(&channel_uuid) {
                            let (sub_key, value) = to_room_value(&self.peer_id, &track_name, track_meta);
                            self.track_pub
                                .insert(channel_uuid, (track_id, self.pubsub_sdk.create_publisher_raw(track_uuid as ChannelUuid, self.data_fb_tx.clone())));

                            //set in room hashmap
                            self.kv_sdk.hset(self.room_key, sub_key, value.clone(), Some(10000));
                            //set in peer hashmap
                            self.kv_sdk.hset(self.peer_key(&self.peer_id), sub_key, value, Some(10000));
                            log::info!("[BlueseaClusterEndpoint] add track {} {track_name} => track_uuid {track_uuid} track_id {track_id}", self.peer_id);
                        } else {
                            log::warn!(
                                "[BlueseaClusterEndpoint] add track but already exist {} {track_name} => track_uuid {track_uuid} track_id {track_id}",
                                self.peer_id
                            );
                        }
                        Ok(())
                    }
                    ClusterRemoteTrackOutgoingEvent::TrackMedia(media_packet) => {
                        if let Some((_, publisher)) = self.track_pub.get(&channel_uuid) {
                            if let Ok(buf) = TrackData::Media(media_packet).try_into() {
                                publisher.send(buf);
                            }
                        } else {
                            log::warn!("[BlueseaClusterEndpoint] send track media but track not found {}", channel_uuid);
                        }
                        Ok(())
                    }
                    ClusterRemoteTrackOutgoingEvent::TrackStats(stats) => {
                        if let Some((_, publisher)) = self.track_pub.get(&channel_uuid) {
                            if let Ok(buf) = TrackData::Stats(stats).try_into() {
                                publisher.send(buf);
                            }
                        } else {
                            log::warn!("[BlueseaClusterEndpoint] send track stats but track not found {}", channel_uuid);
                        }
                        Ok(())
                    }
                    ClusterRemoteTrackOutgoingEvent::TrackRemoved(track_name) => {
                        if self.track_pub.remove(&channel_uuid).is_some() {
                            let sub_key = to_room_key(&self.peer_id, &track_name);

                            //del in room hashmap
                            self.kv_sdk.hdel(self.room_key, sub_key);

                            //del in peer hashmap
                            self.kv_sdk.hdel(self.peer_key(&self.peer_id), sub_key);
                            log::info!("[BlueseaClusterEndpoint] delete track {} {track_name} => track_uuid {track_uuid} track_id {track_id}", self.peer_id);
                        } else {
                            log::warn!(
                                "[BlueseaClusterEndpoint] delete track but not found {} {track_name} => track_uuid {track_uuid} track_id {track_id}",
                                self.peer_id
                            );
                        }
                        Ok(())
                    }
                }
            }
        }
    }

    async fn recv(&mut self) -> Result<ClusterEndpointIncomingEvent, ClusterEndpointError> {
        loop {
            select! {
                event = self.kv_rx.recv().fuse() => match event {
                    Ok((_key, sub_key, value, _ver, _source)) => {
                        if let Some(value) = value { //add or update
                            if let Some((peer, track, meta)) = from_room_value(sub_key, &value) {
                                if self.remote_track_cached.insert(sub_key, (peer.clone(), track.clone())).is_some() {
                                    log::info!("[BlueseaClusterEndpoint] on room update {} {}", peer, track);
                                    return Ok(ClusterEndpointIncomingEvent::PeerTrackUpdated(peer, track, meta));
                                } else {
                                    log::info!("[BlueseaClusterEndpoint] on room add {} {}", peer, track);
                                    return Ok(ClusterEndpointIncomingEvent::PeerTrackAdded(peer, track, meta));
                                }
                            }
                        } else { //delete
                            if let Some((peer, track)) = self.remote_track_cached.remove(&sub_key) {
                                log::info!("[BlueseaClusterEndpoint] on room remove {} {}", peer, track);
                                return Ok(ClusterEndpointIncomingEvent::PeerTrackRemoved(peer, track));
                            }
                        }
                    }
                    Err(_e) => {
                        return Err(ClusterEndpointError::InternalError);
                    }
                },
                event = self.data_fb_rx.recv().fuse() => match event {
                    Ok(fb) => {
                        if let Some((track_id, _)) = self.track_pub.get(&fb.channel.uuid()) {
                            log::trace!("[BlueseaClusterEndpoint] recv track feedback {track_id} => {:?}", fb);
                            match (TrackFeedbackType::try_from(fb.id), fb.feedback_type) {
                                (Ok(TrackFeedbackType::LimitBitrate), FeedbackType::Number { window_ms: _, info }) => {
                                    return Ok(ClusterEndpointIncomingEvent::RemoteTrackEvent(*track_id, ClusterRemoteTrackIncomingEvent::RequestLimitBitrate(info.max as u32)));
                                },
                                (Ok(TrackFeedbackType::RequestKeyFrame), FeedbackType::Number { window_ms: _, info }) => {
                                    let kind = if info.sum > info.count as i64 { //mean has more than 1 has type2 => Pli
                                        transport::RequestKeyframeKind::Pli
                                    } else {
                                        transport::RequestKeyframeKind::Fir
                                    };
                                    return Ok(ClusterEndpointIncomingEvent::RemoteTrackEvent(*track_id, ClusterRemoteTrackIncomingEvent::RequestKeyFrame(kind)));
                                },
                                _ => {}
                            }
                        } else {
                            log::warn!("[BlueseaClusterEndpoint] recv track feedback but track not found {}", fb.channel.uuid());
                        }
                    },
                    Err(_e) => {
                        return Err(ClusterEndpointError::InternalError);
                    }
                },
                event = self.data_rx.recv().fuse() => match event {
                    Ok((sub_id, _node_id, _channel_uuid, data)) => {
                        if let Some(track_id) = self.consumer_map.get(&sub_id) {
                            log::trace!("[BlueseaClusterEndpoint] recv track data {sub_id} => {track_id}");
                            match TrackData::try_from(data) {
                                Ok(TrackData::Media(media_packet)) => {
                                    return Ok(ClusterEndpointIncomingEvent::LocalTrackEvent(*track_id, ClusterLocalTrackIncomingEvent::MediaPacket(media_packet)));
                                },
                                Ok(TrackData::Stats(stats)) => {
                                    return Ok(ClusterEndpointIncomingEvent::LocalTrackEvent(*track_id, ClusterLocalTrackIncomingEvent::MediaStats(stats)));
                                },
                                Err(_e) => {

                                }
                            }
                        } else {
                            log::warn!("[BlueseaClusterEndpoint] recv track data but track not found {}", sub_id);
                        }
                    },
                    Err(_e) => {
                        return Err(ClusterEndpointError::InternalError);
                    }
                }
            }
        }
    }
}

impl Drop for BlueseaClusterEndpoint {
    fn drop(&mut self) {
        assert_eq!(self.consumer_map.len(), 0);
        assert_eq!(self.track_sub_map.len(), 0);
        assert_eq!(self.peer_sub.len(), 0);
        assert_eq!(self.track_pub.len(), 0);
        assert_eq!(self.room_sub, None);
    }
}

//TODO test this
