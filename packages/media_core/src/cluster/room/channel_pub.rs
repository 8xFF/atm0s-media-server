//! Channel Publisher will takecare of pubsub channel for sending data and handle when received channel feedback

use std::{
    collections::{HashMap, VecDeque},
    fmt::Debug,
    hash::Hash,
};

use atm0s_sdn::features::pubsub::{self, ChannelControl, ChannelId, Feedback};
use media_server_protocol::{
    endpoint::{PeerId, TrackMeta, TrackName},
    media::MediaPacket,
};

use crate::{
    cluster::{ClusterEndpointEvent, ClusterRemoteTrackEvent, ClusterRoomHash},
    transport::RemoteTrackId,
};

use super::FeedbackKind;

pub enum Output<Owner> {
    Endpoint(Vec<Owner>, ClusterEndpointEvent),
    Pubsub(pubsub::Control),
}

pub struct RoomChannelPublisher<Owner> {
    room: ClusterRoomHash,
    tracks: HashMap<(Owner, RemoteTrackId), (PeerId, TrackName, ChannelId)>,
    tracks_source: HashMap<ChannelId, (Owner, RemoteTrackId)>,
    queue: VecDeque<Output<Owner>>,
}

impl<Owner: Debug + Hash + Eq + Copy> RoomChannelPublisher<Owner> {
    pub fn new(room: ClusterRoomHash) -> Self {
        Self {
            room,
            tracks: HashMap::new(),
            tracks_source: HashMap::new(),
            queue: VecDeque::new(),
        }
    }

    pub fn on_channel_feedback(&mut self, channel: ChannelId, fb: Feedback) -> Option<Output<Owner>> {
        let fb_kind = FeedbackKind::try_from(fb.kind).ok()?;
        let (owner, track_id) = self.tracks_source.get(&channel)?;
        match fb_kind {
            FeedbackKind::Bitrate => todo!(),
            FeedbackKind::KeyFrameRequest => Some(Output::Endpoint(
                vec![owner.clone()],
                ClusterEndpointEvent::RemoteTrack(*track_id, ClusterRemoteTrackEvent::RequestKeyFrame),
            )),
        }
    }

    pub fn on_track_publish(&mut self, owner: Owner, track: RemoteTrackId, peer: PeerId, name: TrackName) -> Option<Output<Owner>> {
        log::info!("[ClusterRoom {}] peer ({peer} started track {name})", self.room);
        let channel_id = super::gen_channel_id(self.room, &peer, &name);
        self.tracks.insert((owner, track), (peer.clone(), name.clone(), channel_id));
        self.tracks_source.insert(channel_id, (owner, track));

        Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::PubStart)))
    }

    pub fn on_track_data(&mut self, owner: Owner, track: RemoteTrackId, media: MediaPacket) -> Option<Output<Owner>> {
        log::trace!("[ClusterRoom {}] peer {:?} track {track} publish media payload {} seq {}", self.room, owner, media.pt, media.seq);
        let (_peer, _name, channel_id) = self.tracks.get(&(owner, track))?;
        let data = media.serialize();
        Some(Output::Pubsub(pubsub::Control(*channel_id, ChannelControl::PubData(data))))
    }

    pub fn on_track_unpublish(&mut self, owner: Owner, track: RemoteTrackId) -> Option<Output<Owner>> {
        let (peer, name, channel_id) = self.tracks.remove(&(owner, track))?;
        self.tracks_source.remove(&channel_id).expect("Should have track_source");
        log::info!("[ClusterRoom {}] peer ({peer} stopped track {name})", self.room);
        Some(Output::Pubsub(pubsub::Control(channel_id, ChannelControl::PubStop)))
    }

    pub fn pop_output(&mut self) -> Option<Output<Owner>> {
        self.queue.pop_front()
    }
}

#[cfg(test)]
mod tests {
    //TODO Track start => should register with SDN
    //TODO Track stop => should unregister with SDN
    //TODO Track media => should send data over SDN
    //TODO Handle feedback: should handle KeyFrame feedback
    //TODO Handle feedback: should handle Bitrate feedback
}
