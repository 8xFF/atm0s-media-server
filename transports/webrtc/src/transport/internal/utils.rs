use str0m::media::MediaKind;

pub fn to_transport_kind(value: MediaKind) -> transport::MediaKind {
    match value {
        MediaKind::Audio => transport::MediaKind::Audio,
        MediaKind::Video => transport::MediaKind::Video,
    }
}

/// Convert a SDP patch to ICE candidates.
/// When received:
///
/// ```
/// a=ice-ufrag:EsAw
/// a=ice-pwd:P2uYro0UCOQ4zxjKXaWCBui1
/// a=group:BUNDLE 0 1
/// m=audio 9 UDP/TLS/RTP/SAVPF 111
/// a=mid:0
/// a=candidate:1387637174 1 udp 2122260223 192.0.2.1 61764 typ host generation 0 ufrag EsAw network-id 1
/// a=candidate:3471623853 1 udp 2122194687 198.51.100.1 61765 typ host generation 0 ufrag EsAw network-id 2
/// a=candidate:473322822 1 tcp 1518280447 192.0.2.1 9 typ host tcptype active generation 0 ufrag EsAw network-id 1
/// a=candidate:2154773085 1 tcp 1518214911 198.51.100.2 9 typ host tcptype active generation 0 ufrag EsAw network-id 2
/// a=end-of-candidates
/// ```
///
/// Should return list of ICE candidates:
///
/// ```
/// [
///    "candidate:1387637174 1 udp 2122260223 192.0.2.1 61764 typ host generation 0 ufrag EsAw network-id 1",
///    ....
/// ]
/// ```
pub fn sdp_patch_to_ices(patch: &str) -> Vec<String> {
    let mut candidates = vec![];
    for line in patch.lines() {
        if line.starts_with("a=candidate:") {
            candidates.push(line[2..].to_owned());
        }
    }
    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_transport_kind() {
        assert_eq!(to_transport_kind(MediaKind::Audio), transport::MediaKind::Audio);
        assert_eq!(to_transport_kind(MediaKind::Video), transport::MediaKind::Video);
    }

    #[test]
    fn test_sdp_patch_to_ices() {
        let patch = "a=ice-ufrag:EsAw\n\
                     a=ice-pwd:P2uYro0UCOQ4zxjKXaWCBui1\n\
                     a=group:BUNDLE 0 1\n\
                     m=audio 9 UDP/TLS/RTP/SAVPF 111\n\
                     a=mid:0\n\
                     a=candidate:1387637174 1 udp 2122260223 192.0.2.1 61764 typ host generation 0 ufrag EsAw network-id 1\n\
                     a=candidate:3471623853 1 udp 2122194687 198.51.100.1 61765 typ host generation 0 ufrag EsAw network-id 2\n\
                     a=candidate:473322822 1 tcp 1518280447 192.0.2.1 9 typ host tcptype active generation 0 ufrag EsAw network-id 1\n\
                     a=candidate:2154773085 1 tcp 1518214911 198.51.100.2 9 typ host tcptype active generation 0 ufrag EsAw network-id 2\n\
                     a=end-of-candidates";

        let expected_candidates = vec![
            "candidate:1387637174 1 udp 2122260223 192.0.2.1 61764 typ host generation 0 ufrag EsAw network-id 1".to_owned(),
            "candidate:3471623853 1 udp 2122194687 198.51.100.1 61765 typ host generation 0 ufrag EsAw network-id 2".to_owned(),
            "candidate:473322822 1 tcp 1518280447 192.0.2.1 9 typ host tcptype active generation 0 ufrag EsAw network-id 1".to_owned(),
            "candidate:2154773085 1 tcp 1518214911 198.51.100.2 9 typ host tcptype active generation 0 ufrag EsAw network-id 2".to_owned(),
        ];

        assert_eq!(sdp_patch_to_ices(patch), expected_candidates);
    }
}
