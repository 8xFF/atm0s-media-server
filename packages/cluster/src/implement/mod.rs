mod endpoint;
mod rpc;
mod secure;
mod server;
mod types;

pub use atm0s_sdn::{NodeAddr, NodeId};
pub use secure::*;
pub use server::{ServerSdn, ServerSdnConfig};

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use async_std::prelude::FutureExt;
    use atm0s_sdn::OptionUtils;
    use transport::{MediaKind, MediaPacket};

    use crate::{
        rpc::{RpcEmitter, RpcEndpoint, RpcRequest},
        Cluster, ClusterEndpoint, ClusterEndpointIncomingEvent, ClusterEndpointOutgoingEvent, ClusterLocalTrackIncomingEvent, ClusterLocalTrackOutgoingEvent, ClusterRemoteTrackOutgoingEvent,
        ClusterTrackMeta, ClusterTrackScalingType, ClusterTrackStatus, ClusterTrackUuid,
    };

    use super::{ServerSdn, ServerSdnConfig};

    #[async_std::test]
    async fn subscribe_room() {
        let (mut server, _rpc) = ServerSdn::new(
            1,
            0,
            100,
            ServerSdnConfig {
                secret: "static_key".to_string(),
                seeds: vec![],
                connect_tags: vec!["local".to_string()],
                local_tags: vec!["local".to_string()],
            },
        )
        .await;

        let mut peer1 = server.build("room1", "peer1");
        let mut peer2 = server.build("room1", "peer2");

        peer1.on_event(ClusterEndpointOutgoingEvent::SubscribeRoomStreams).expect("");
        peer2.on_event(ClusterEndpointOutgoingEvent::SubscribeRoomStreams).expect("");

        // add track to peer2 then should fire event to both peer1 and peer2
        let peer2_cluster_track_uuid = ClusterTrackUuid::from_info("room1", "peer2", "audio_main");
        peer2
            .on_event(ClusterEndpointOutgoingEvent::RemoteTrackEvent(
                10,
                peer2_cluster_track_uuid,
                ClusterRemoteTrackOutgoingEvent::TrackAdded(
                    "audio_main".to_string(),
                    ClusterTrackMeta {
                        kind: MediaKind::Audio,
                        scaling: ClusterTrackScalingType::Single,
                        layers: vec![],
                        status: ClusterTrackStatus::Connected,
                        active: true,
                        label: None,
                    },
                ),
            ))
            .expect("");

        let event1 = peer1.recv().await.expect("");
        let event2 = peer2.recv().await.expect("");
        assert_eq!(event1, event2);
        assert_eq!(
            event1,
            ClusterEndpointIncomingEvent::PeerTrackAdded(
                "peer2".to_string(),
                "audio_main".to_string(),
                ClusterTrackMeta {
                    kind: MediaKind::Audio,
                    scaling: ClusterTrackScalingType::Single,
                    layers: vec![],
                    status: ClusterTrackStatus::Connected,
                    active: true,
                    label: None,
                }
            )
        );

        // remove track from peer2 then should fire event to both peer1 and peer2
        peer2
            .on_event(ClusterEndpointOutgoingEvent::RemoteTrackEvent(
                10,
                peer2_cluster_track_uuid,
                ClusterRemoteTrackOutgoingEvent::TrackRemoved("audio_main".to_string()),
            ))
            .expect("");

        let event1 = peer1.recv().await.expect("");
        let event2 = peer2.recv().await.expect("");
        assert_eq!(event1, event2);
        assert_eq!(event1, ClusterEndpointIncomingEvent::PeerTrackRemoved("peer2".to_string(), "audio_main".to_string()));

        peer1.on_event(ClusterEndpointOutgoingEvent::UnsubscribeRoomStreams).expect("");
        peer2.on_event(ClusterEndpointOutgoingEvent::UnsubscribeRoomStreams).expect("");
    }

    #[async_std::test]
    async fn subscribe_peer() {
        let (mut server, _rpc) = ServerSdn::new(
            2,
            0,
            100,
            ServerSdnConfig {
                secret: "static_key".to_string(),
                seeds: vec![],
                connect_tags: vec!["local".to_string()],
                local_tags: vec!["local".to_string()],
            },
        )
        .await;

        let mut peer1 = server.build("room1", "peer1");
        let mut peer2 = server.build("room1", "peer2");

        peer1.on_event(ClusterEndpointOutgoingEvent::SubscribeSinglePeer("peer2".to_string())).expect("");
        peer2.on_event(ClusterEndpointOutgoingEvent::SubscribeSinglePeer("peer1".to_string())).expect("");

        // add track to peer2 then should fire event to only peer1
        let peer2_cluster_track_uuid = ClusterTrackUuid::from_info("room1", "peer2", "audio_main");
        peer2
            .on_event(ClusterEndpointOutgoingEvent::RemoteTrackEvent(
                10,
                peer2_cluster_track_uuid,
                ClusterRemoteTrackOutgoingEvent::TrackAdded(
                    "audio_main".to_string(),
                    ClusterTrackMeta {
                        kind: MediaKind::Audio,
                        scaling: ClusterTrackScalingType::Single,
                        layers: vec![],
                        status: ClusterTrackStatus::Connected,
                        active: true,
                        label: None,
                    },
                ),
            ))
            .expect("");

        let event1 = peer1.recv().timeout(Duration::from_millis(100)).await;
        let event2 = peer2.recv().timeout(Duration::from_millis(100)).await;
        assert_eq!(
            event1,
            Ok(Ok(ClusterEndpointIncomingEvent::PeerTrackAdded(
                "peer2".to_string(),
                "audio_main".to_string(),
                ClusterTrackMeta {
                    kind: MediaKind::Audio,
                    scaling: ClusterTrackScalingType::Single,
                    layers: vec![],
                    status: ClusterTrackStatus::Connected,
                    active: true,
                    label: None,
                }
            )))
        );
        assert!(event2.is_err());

        // remove track from peer2 then should fire event to only peer1
        peer2
            .on_event(ClusterEndpointOutgoingEvent::RemoteTrackEvent(
                10,
                peer2_cluster_track_uuid,
                ClusterRemoteTrackOutgoingEvent::TrackRemoved("audio_main".to_string()),
            ))
            .expect("");

        let event1 = peer1.recv().timeout(Duration::from_millis(100)).await;
        let event2 = peer2.recv().timeout(Duration::from_millis(100)).await;
        assert_eq!(event1, Ok(Ok(ClusterEndpointIncomingEvent::PeerTrackRemoved("peer2".to_string(), "audio_main".to_string()))));
        assert!(event2.is_err());

        peer1.on_event(ClusterEndpointOutgoingEvent::UnsubscribeSinglePeer("peer2".to_string())).expect("");
        peer2.on_event(ClusterEndpointOutgoingEvent::UnsubscribeSinglePeer("peer1".to_string())).expect("");
    }

    #[async_std::test]
    async fn subscribe_stream() {
        let (mut server, _rpc) = ServerSdn::new(
            3,
            0,
            100,
            ServerSdnConfig {
                secret: "static_key".to_string(),
                seeds: vec![],
                connect_tags: vec!["local".to_string()],
                local_tags: vec!["local".to_string()],
            },
        )
        .await;

        let mut peer1 = server.build("room1", "peer1");
        let mut peer2 = server.build("room1", "peer2");
        let peer2_cluster_track_uuid = ClusterTrackUuid::from_info("room1", "peer2", "audio_main");

        peer1
            .on_event(ClusterEndpointOutgoingEvent::LocalTrackEvent(
                1,
                ClusterLocalTrackOutgoingEvent::Subscribe("peer2".to_string(), "audio_main".to_string()),
            ))
            .expect("");
        peer2
            .on_event(ClusterEndpointOutgoingEvent::RemoteTrackEvent(
                10,
                peer2_cluster_track_uuid,
                ClusterRemoteTrackOutgoingEvent::TrackAdded(
                    "audio_main".to_string(),
                    ClusterTrackMeta {
                        kind: MediaKind::Audio,
                        scaling: ClusterTrackScalingType::Single,
                        layers: vec![],
                        status: ClusterTrackStatus::Connected,
                        active: true,
                        label: None,
                    },
                ),
            ))
            .expect("");

        async_std::task::sleep(Duration::from_millis(300)).await;

        //peer2 fire media packet to channel, then peer1 should receive it
        let pkt = MediaPacket::simple_audio(1, 1000, vec![1, 2, 3]);
        peer2
            .on_event(ClusterEndpointOutgoingEvent::RemoteTrackEvent(
                10,
                peer2_cluster_track_uuid,
                ClusterRemoteTrackOutgoingEvent::TrackMedia(pkt.clone()),
            ))
            .expect("");

        let event1 = peer1.recv().timeout(Duration::from_millis(100)).await.expect("").expect("");
        assert_eq!(
            event1,
            ClusterEndpointIncomingEvent::LocalTrackEvent(1, ClusterLocalTrackIncomingEvent::MediaPacket(peer2_cluster_track_uuid, pkt))
        );

        peer1
            .on_event(ClusterEndpointOutgoingEvent::LocalTrackEvent(
                1,
                ClusterLocalTrackOutgoingEvent::Unsubscribe("peer2".to_string(), "audio_main".to_string()),
            ))
            .expect("");
        peer2
            .on_event(ClusterEndpointOutgoingEvent::RemoteTrackEvent(
                10,
                peer2_cluster_track_uuid,
                ClusterRemoteTrackOutgoingEvent::TrackRemoved("audio_main".to_string()),
            ))
            .expect("");
    }

    #[async_std::test]
    async fn rpc() {
        let (_server, mut rpc) = ServerSdn::new(
            4,
            0,
            100,
            ServerSdnConfig {
                secret: "static_key".to_string(),
                seeds: vec![],
                connect_tags: vec!["local".to_string()],
                local_tags: vec!["local".to_string()],
            },
        )
        .await;

        let emitter = rpc.emitter();
        let task = async_std::task::spawn(async move {
            while let Some(req) = rpc.recv().await {
                assert_eq!(req.cmd(), "DEMO");
                let req2 = req.parse::<Vec<u8>, Vec<u8>>().expect("should parse");
                assert_eq!(req2.param(), &vec![1, 2, 3]);
                req2.answer(Ok(vec![4, 5, 6]));
            }
        });

        let res = emitter.request::<_, Vec<u8>>(100, None, "DEMO", vec![1, 2, 3], 1000).await;
        assert_eq!(res, Ok(vec![4, 5, 6]));
        task.cancel().await.print_none("should cancel");
    }
}
