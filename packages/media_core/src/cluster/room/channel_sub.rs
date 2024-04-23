//! Channel Subscriber handle logic for viewer. This module takecare sending Sub or Unsub, and also feedback
//!

use std::{
    collections::{HashMap, VecDeque},
    fmt::Debug,
    hash::Hash,
    time::Instant,
};

use atm0s_sdn::{
    features::pubsub::{self, ChannelControl, ChannelId, Feedback},
    NodeId,
};
use media_server_protocol::{
    endpoint::{PeerId, TrackName},
    media::MediaPacket,
};

use crate::{
    cluster::{room::FeedbackKind, ClusterEndpointEvent, ClusterLocalTrackEvent, ClusterRoomHash},
    transport::LocalTrackId,
};

pub enum Output<Owner> {
    Endpoint(Vec<Owner>, ClusterEndpointEvent),
    Pubsub(pubsub::Control),
}

pub struct RoomChannelSubscribe<Owner> {
    room: ClusterRoomHash,
    subscribers: HashMap<ChannelId, Vec<(Owner, LocalTrackId)>>,
    subscribers_source: HashMap<(Owner, LocalTrackId), (ChannelId, PeerId, TrackName)>,
    queue: VecDeque<Output<Owner>>,
}

impl<Owner: Hash + Eq + Copy + Debug> RoomChannelSubscribe<Owner> {
    pub fn new(room: ClusterRoomHash) -> Self {
        Self {
            room,
            subscribers: HashMap::new(),
            subscribers_source: HashMap::new(),
            queue: VecDeque::new(),
        }
    }

    pub fn on_channel_relay_changed(&mut self, channel: ChannelId, _relay: NodeId) -> Option<Output<Owner>> {
        let subscribers = self.subscribers.get(&channel)?;
        log::info!("[ClusterRoom {}] cluster: channel {channel} source changed => fire event to {:?}", self.room, subscribers);
        for (owner, track) in subscribers {
            self.queue
                .push_back(Output::Endpoint(vec![*owner], ClusterEndpointEvent::LocalTrack(*track, ClusterLocalTrackEvent::SourceChanged)))
        }
        self.queue.pop_front()
    }

    pub fn on_channel_data(&mut self, channel: ChannelId, data: Vec<u8>) -> Option<Output<Owner>> {
        let pkt = MediaPacket::deserialize(&data)?;
        let subscribers = self.subscribers.get(&channel)?;
        log::trace!("[ClusterRoom {}] on channel media payload {} seq {} to {} subscribers", self.room, pkt.pt, pkt.seq, subscribers.len());
        for (owner, track) in subscribers {
            self.queue
                .push_back(Output::Endpoint(vec![*owner], ClusterEndpointEvent::LocalTrack(*track, ClusterLocalTrackEvent::Media(pkt.clone()))))
        }
        self.queue.pop_front()
    }

    pub fn on_track_subscribe(&mut self, owner: Owner, track: LocalTrackId, target_peer: PeerId, target_track: TrackName) -> Option<Output<Owner>> {
        let channel_id: ChannelId = super::gen_channel_id(self.room, &target_peer, &target_track);
        log::info!(
            "[ClusterRoom {}] owner {:?} track {track} subscribe peer {target_peer} track {target_track}), channel: {channel_id}",
            self.room,
            owner
        );
        self.subscribers_source.insert((owner, track), (channel_id, target_peer, target_track));
        let subscribers = self.subscribers.entry(channel_id).or_insert(Default::default());
        subscribers.push((owner, track));
        if subscribers.len() == 1 {
            log::info!("[ClusterRoom {}] first subscriber => Sub channel {channel_id}", self.room);
            Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::SubAuto)))
        } else {
            None
        }
    }

    pub fn on_track_request_key(&mut self, owner: Owner, track: LocalTrackId) -> Option<Output<Owner>> {
        let (channel_id, peer, track) = self.subscribers_source.get(&(owner, track))?;
        log::info!("[ClusterRoom {}] request key-frame {channel_id} {peer} {track}", self.room);
        Some(Output::Pubsub(pubsub::Control(
            *channel_id,
            ChannelControl::FeedbackAuto(Feedback::simple(FeedbackKind::KeyFrameRequest as u8, 1, 100, 200)),
        )))
    }

    pub fn on_track_desired_bitrate(&mut self, owner: Owner, track: LocalTrackId, bitrate: u32) -> Option<Output<Owner>> {
        todo!()
    }

    pub fn on_track_unsubscribe(&mut self, owner: Owner, track: LocalTrackId) -> Option<Output<Owner>> {
        let (channel_id, target_peer, target_track) = self.subscribers_source.get(&(owner, track))?;
        log::info!(
            "[ClusterRoom {}] owner {:?} track {track} unsubscribe from source {target_peer} {target_track}, channel {channel_id}",
            self.room,
            owner
        );
        let subscribers = self.subscribers.get_mut(channel_id)?;
        let (index, _) = subscribers.iter().enumerate().find(|e| e.1.eq(&(owner, track)))?;
        subscribers.swap_remove(index);

        if subscribers.is_empty() {
            log::info!("[ClusterRoom {}] last unsubscriber => Unsub channel {channel_id}", self.room);
            Some(Output::Pubsub(pubsub::Control(*channel_id, ChannelControl::UnsubAuto)))
        } else {
            None
        }
    }

    pub fn pop_output(&mut self, now: Instant) -> Option<Output<Owner>> {
        self.queue.pop_front()
    }
}

#[cfg(test)]
mod tests {
    //TODO First Subscribe channel should sending Sub
    //TODO Last Unsubscribe channel should sending Unsub
    //TODO Sending key-frame request
    //TODO Sending bitrate request single sub
    //TODO Sending bitrate request multi subs
}
