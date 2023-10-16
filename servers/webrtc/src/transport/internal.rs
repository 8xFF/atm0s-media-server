use std::collections::{HashMap, VecDeque};

use endpoint::{
    rpc::{LocalTrackRpcIn, LocalTrackRpcOut, RemoteTrackRpcIn, RemoteTrackRpcOut},
    EndpointRpcIn, EndpointRpcOut,
};
use str0m::{channel::ChannelId, media::Direction, Event};
use transport::{
    LocalTrackIncomingEvent, LocalTrackOutgoingEvent, MediaPacket, MediaPacketExtensions, MediaSampleRate, RemoteTrackIncomingEvent, RemoteTrackOutgoingEvent, TrackId, TrackMeta, TransportError,
    TransportIncomingEvent, TransportOutgoingEvent,
};

use self::{
    life_cycle::{life_cycle_event_to_event, TransportLifeCycle},
    local_track_id_generator::LocalTrackIdGenerator,
    mid_history::MidHistory,
    msid_alias::MsidAlias,
    rpc::{rpc_from_string, rpc_local_track_to_string, rpc_remote_track_to_string, rpc_to_string, IncomingRpc},
    utils::{to_transport_kind, track_to_mid},
};
use crate::rpc::WebrtcConnectRequestSender;

use super::{Str0mAction, WebrtcTransportEvent};

pub(crate) mod life_cycle;
mod local_track_id_generator;
mod mid_history;
mod msid_alias;
mod rpc;
mod utils;

pub struct WebrtcTransportInternal<L>
where
    L: TransportLifeCycle,
{
    life_cycle: L,
    msid_alias: MsidAlias,
    mid_history: MidHistory,
    local_track_id_map: HashMap<String, TrackId>,
    remote_track_id_map: HashMap<String, TrackId>,
    local_track_id_gen: LocalTrackIdGenerator,
    channel_id: Option<ChannelId>,
    channel_pending_msgs: VecDeque<String>,
    endpoint_actions: VecDeque<Result<TransportIncomingEvent<EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn>, TransportError>>,
    str0m_actions: VecDeque<Str0mAction>,
}

impl<L> WebrtcTransportInternal<L>
where
    L: TransportLifeCycle,
{
    pub fn new(life_cycle: L) -> Self {
        log::info!("[TransportWebrtcInternal] created");

        Self {
            life_cycle,
            msid_alias: Default::default(),
            mid_history: Default::default(),
            local_track_id_map: Default::default(),
            remote_track_id_map: Default::default(),
            local_track_id_gen: Default::default(),
            channel_id: None,
            channel_pending_msgs: Default::default(),
            endpoint_actions: Default::default(),
            str0m_actions: Default::default(),
        }
    }

    pub fn map_remote_stream(&mut self, sender: WebrtcConnectRequestSender) {
        self.msid_alias.add_alias(&sender.uuid, &sender.label, &sender.kind, &sender.name);
    }

    fn send_msg(&mut self, msg: String) {
        if let Some(channel_id) = self.channel_id {
            self.str0m_actions.push_back(Str0mAction::Datachannel(channel_id, msg));
        } else {
            self.channel_pending_msgs.push_back(msg);
        }
    }

    fn restore_msgs(&mut self) {
        assert!(self.channel_id.is_some());
        while let Some(msg) = self.channel_pending_msgs.pop_front() {
            if let Some(channel_id) = self.channel_id {
                self.str0m_actions.push_back(Str0mAction::Datachannel(channel_id, msg));
            }
        }
    }

    pub fn on_tick(&mut self, now_ms: u64) -> Result<(), TransportError> {
        if let Some(e) = self.life_cycle.on_tick(now_ms) {
            log::info!("[TransportWebrtc] on new state on tick {:?}", e);
            self.endpoint_actions.push_back(life_cycle_event_to_event(Some(e)));
        }
        Ok(())
    }

    pub fn on_endpoint_event(&mut self, _now_ms: u64, event: TransportOutgoingEvent<EndpointRpcOut, RemoteTrackRpcOut, LocalTrackRpcOut>) -> Result<(), TransportError> {
        match event {
            TransportOutgoingEvent::LocalTrackEvent(track_id, event) => match event {
                LocalTrackOutgoingEvent::MediaPacket(pkt) => {
                    let mid = track_to_mid(track_id);
                    self.str0m_actions.push_back(Str0mAction::Media(mid, pkt));
                }
                LocalTrackOutgoingEvent::Rpc(rpc) => {
                    let msg = rpc_local_track_to_string(rpc);
                    log::info!("[TransportWebrtc] on local track out rpc: {}", msg);
                    self.send_msg(msg);
                }
            },
            TransportOutgoingEvent::RemoteTrackEvent(track_id, event) => match event {
                RemoteTrackOutgoingEvent::RequestKeyFrame => {
                    let mid = track_to_mid(track_id);
                    self.str0m_actions.push_back(Str0mAction::RequestKeyFrame(mid));
                }
                RemoteTrackOutgoingEvent::Rpc(rpc) => {
                    let msg = rpc_remote_track_to_string(rpc);
                    log::info!("[TransportWebrtc] on remote track out rpc: {}", msg);
                    self.send_msg(msg);
                }
            },
            TransportOutgoingEvent::RequestLimitBitrate(bitrate) => {
                //TODO
            }
            TransportOutgoingEvent::Rpc(rpc) => {
                let msg = rpc_to_string(rpc);
                log::info!("[TransportWebrtc] on endpoint out rpc: {}", msg);
                self.send_msg(msg);
            }
        }
        Ok(())
    }

    pub fn on_custom_event(&mut self, now_ms: u64, event: WebrtcTransportEvent) -> Result<(), TransportError> {
        Ok(())
    }

    pub fn on_str0m_event(&mut self, now_ms: u64, event: str0m::Event) {
        match event {
            Event::Connected => {
                self.endpoint_actions.push_back(life_cycle_event_to_event(self.life_cycle.on_webrtc_connected(now_ms)));
            }
            Event::ChannelOpen(chanel_id, _name) => {
                self.channel_id = Some(chanel_id);
                self.restore_msgs();
                self.endpoint_actions.push_back(life_cycle_event_to_event(self.life_cycle.on_data_channel(now_ms, true)));
            }
            Event::ChannelData(data) => {
                if !data.binary {
                    if let Ok(data) = String::from_utf8(data.data) {
                        match rpc_from_string(&data) {
                            Ok(IncomingRpc::Endpoint(rpc)) => {
                                log::info!("[TransportWebrtcInternal] on incoming endpoint rpc: [{:?}]", rpc);
                                self.endpoint_actions.push_back(Ok(TransportIncomingEvent::Rpc(rpc)));
                            }
                            Ok(IncomingRpc::LocalTrack(track_name, rpc)) => {
                                if let Some(track_id) = self.local_track_id_map.get(&track_name) {
                                    log::info!("[TransportWebrtcInternal] on incoming local track[{}] rpc: [{:?}]", track_name, rpc);
                                    self.endpoint_actions
                                        .push_back(Ok(TransportIncomingEvent::LocalTrackEvent(*track_id, LocalTrackIncomingEvent::Rpc(rpc))));
                                } else {
                                    log::warn!("[TransportWebrtcInternal] on incoming local invalid track[{}] rpc: [{:?}]", track_name, rpc);
                                }
                            }
                            Ok(IncomingRpc::RemoteTrack(track_name, rpc)) => {
                                if let Some(track_id) = self.remote_track_id_map.get(&track_name) {
                                    log::info!("[TransportWebrtcInternal] on incoming remote track[{}] rpc: [{:?}]", track_name, rpc);
                                    self.endpoint_actions
                                        .push_back(Ok(TransportIncomingEvent::RemoteTrackEvent(*track_id, RemoteTrackIncomingEvent::Rpc(rpc))));
                                } else {
                                    log::warn!("[TransportWebrtcInternal] on incoming remote invalid track[{}] rpc: [{:?}]", track_name, rpc);
                                }
                            }
                            _ => {
                                log::warn!("[TransportWebrtcInternal] invalid rpc: {}", data);
                            }
                        }
                    }
                }
            }
            Event::ChannelClose(_chanel_id) => {
                self.endpoint_actions.push_back(life_cycle_event_to_event(self.life_cycle.on_data_channel(now_ms, false)));
            }
            Event::IceConnectionStateChange(state) => {
                self.endpoint_actions.push_back(life_cycle_event_to_event(self.life_cycle.on_ice_state(now_ms, state)));
            }
            Event::RtpPacket(rtp) => {
                let track_id = rtp.header.ext_vals.mid.map(|mid| utils::mid_to_track(&mid));
                let ssrc: &u32 = &rtp.header.ssrc;
                if let Some(track_id) = self.mid_history.get(track_id, *ssrc) {
                    // log::info!("on rtp {} => {}", rtp.header.ssrc, track_id);
                    self.endpoint_actions.push_back(Ok(TransportIncomingEvent::RemoteTrackEvent(
                        track_id,
                        RemoteTrackIncomingEvent::MediaPacket(MediaPacket {
                            pt: *(&rtp.header.payload_type as &u8),
                            seq_no: rtp.header.sequence_number,
                            time: rtp.header.timestamp,
                            marker: rtp.header.marker,
                            ext_vals: MediaPacketExtensions {
                                abs_send_time: rtp.header.ext_vals.abs_send_time.map(|t| (t.numer(), t.denom())),
                                transport_cc: rtp.header.ext_vals.transport_cc,
                            },
                            nackable: true,
                            payload: rtp.payload,
                        }),
                    )));
                } else {
                    log::warn!("on rtp without mid {}", rtp.header.ssrc);
                }
            }
            Event::MediaAdded(added) => {
                match added.direction {
                    Direction::RecvOnly => {
                        //remote stream
                        let track_id = utils::mid_to_track(&added.mid);
                        if let Some(info) = self.msid_alias.get_alias(&added.msid.stream_id, &added.msid.track_id) {
                            self.remote_track_id_map.insert(info.name.clone(), track_id);
                            log::info!("[TransportWebrtcInternal] added remote track {} => {} added {:?} {:?}", info.name, track_id, added, info);
                            self.endpoint_actions.push_back(Ok(TransportIncomingEvent::RemoteTrackAdded(
                                info.name,
                                track_id,
                                TrackMeta {
                                    kind: to_transport_kind(added.kind),
                                    sample_rate: MediaSampleRate::HzCustom(0), //TODO
                                    label: Some(info.label),
                                },
                            )));
                        }
                    }
                    Direction::SendOnly => {
                        //local stream
                        let track_id = utils::mid_to_track(&added.mid);
                        let track_name = self.local_track_id_gen.generate(added.kind, added.mid);
                        log::info!("[TransportWebrtcInternal] added local track {} => {} added {:?}", track_name, track_id, added);
                        self.local_track_id_map.insert(track_name.clone(), track_id);
                        self.endpoint_actions.push_back(Ok(TransportIncomingEvent::LocalTrackAdded(
                            track_name,
                            track_id,
                            TrackMeta {
                                kind: to_transport_kind(added.kind),
                                sample_rate: MediaSampleRate::HzCustom(0), //TODO
                                label: None,
                            },
                        )));
                    }
                    _ => {
                        panic!("not supported")
                    }
                }
            }
            Event::MediaChanged(media) => {
                //TODO
            }
            Event::StreamPaused(paused) => {
                //TODO
            }
            Event::KeyframeRequest(req) => {
                //TODO
                let track_id = utils::mid_to_track(&req.mid);
                self.endpoint_actions
                    .push_back(Ok(TransportIncomingEvent::LocalTrackEvent(track_id, LocalTrackIncomingEvent::RequestKeyFrame)));
            }
            _ => {}
        }
    }

    pub fn endpoint_action(&mut self) -> Option<Result<TransportIncomingEvent<EndpointRpcIn, RemoteTrackRpcIn, LocalTrackRpcIn>, TransportError>> {
        self.endpoint_actions.pop_front()
    }

    pub fn str0m_action(&mut self) -> Option<Str0mAction> {
        self.str0m_actions.pop_front()
    }
}

impl<L: TransportLifeCycle> Drop for WebrtcTransportInternal<L> {
    fn drop(&mut self) {
        log::info!("[TransportWebrtcInternal] drop");
    }
}

#[cfg(test)]
mod test {
    //TODO test this
}
