use std::collections::HashSet;
use std::io::{Read, Seek, SeekFrom, Write};

// EBML/WebM element IDs used by the cue repair pass.
//
// References:
// - Matroska element table: https://www.matroska.org/technical/elements.html
// - Matroska notes on random access points/keyframes:
//   https://www.matroska.org/technical/notes.html
// - RFC 9559 Cues guidance:
//   https://www.rfc-editor.org/rfc/rfc9559.html#name-cues
const ID_VOID: u64 = 0xec;
const ID_SEGMENT: u64 = 0x18538067;
const ID_CLUSTER: u64 = 0x1f43b675;
const ID_CUES: u64 = 0x1c53bb6b;
const ID_CUE_POINT: u64 = 0xbb;
const ID_CUE_TRACK_POSITIONS: u64 = 0xb7;
const ID_CUE_CLUSTER_POSITION: u64 = 0xf1;
const ID_SIMPLE_BLOCK: u64 = 0xa3;

#[derive(Debug, Clone, Copy)]
struct Element {
    id: u64,
    start: usize,
    data_start: usize,
    data_end: usize,
    size_len: usize,
}

impl Element {
    fn end(&self) -> usize {
        self.data_end
    }

    fn total_len(&self) -> usize {
        self.data_end - self.start
    }
}

pub fn repair_cues_for_seekable_clusters<W>(writer: &mut W) -> std::io::Result<bool>
where
    W: Read + Write + Seek,
{
    let mut data = Vec::new();
    writer.seek(SeekFrom::Start(0))?;
    writer.read_to_end(&mut data)?;

    let Some(repaired) = repair_cues_bytes(&data) else {
        return Ok(false);
    };

    writer.seek(SeekFrom::Start(0))?;
    writer.write_all(&repaired)?;
    writer.flush()?;
    Ok(true)
}

fn repair_cues_bytes(data: &[u8]) -> Option<Vec<u8>> {
    let segment = find_element(data, 0, data.len(), ID_SEGMENT)?;
    let cues = find_element(data, segment.data_start, segment.data_end, ID_CUES)?;
    let seekable_clusters = seekable_cluster_positions(data, segment)?;
    let cue_points = collect_repaired_cue_points(data, cues, &seekable_clusters)?;
    let repaired_payload_len: usize = cue_points.iter().map(|point| point.len()).sum();
    let original_payload_len = cues.data_end - cues.data_start;

    if repaired_payload_len == original_payload_len {
        return None;
    }

    let mut repaired_cues = Vec::with_capacity(cues.total_len());
    repaired_cues.extend_from_slice(&data[cues.start..cues.data_start - cues.size_len]);
    write_vint_size(repaired_payload_len, cues.size_len, &mut repaired_cues)?;
    for point in cue_points {
        repaired_cues.extend_from_slice(point);
    }

    let padding_len = cues.total_len().checked_sub(repaired_cues.len())?;
    repaired_cues.extend_from_slice(&void_element(padding_len)?);
    if repaired_cues.len() != cues.total_len() {
        return None;
    }

    let mut out = data.to_vec();
    out[cues.start..cues.end()].copy_from_slice(&repaired_cues);
    Some(out)
}

fn seekable_cluster_positions(data: &[u8], segment: Element) -> Option<HashSet<u64>> {
    let mut clusters = HashSet::new();
    let mut pos = segment.data_start;
    while pos < segment.data_end {
        let element = read_element(data, pos)?;
        // Browser/range seeks use CueClusterPosition to jump to a Cluster.
        // If that Cluster starts with a non-key VP8 frame, decoding cannot
        // produce the requested frame until a later keyframe arrives. WebM's
        // container guidelines explicitly call out key-frame Cues for seeking:
        // https://www.webmproject.org/docs/container/#implementation-details
        if element.id == ID_CLUSTER && cluster_starts_with_keyframe(data, element)? {
            clusters.insert((element.start - segment.data_start) as u64);
        }
        pos = element.end();
    }
    Some(clusters)
}

fn cluster_starts_with_keyframe(data: &[u8], cluster: Element) -> Option<bool> {
    let mut pos = cluster.data_start;
    while pos < cluster.data_end {
        let element = read_element(data, pos)?;
        if element.id == ID_SIMPLE_BLOCK {
            return simple_block_is_keyframe(&data[element.data_start..element.data_end]);
        }
        pos = element.end();
    }
    Some(false)
}

fn simple_block_is_keyframe(block: &[u8]) -> Option<bool> {
    let (_, track_len) = read_vint(block, 0)?;
    let flags_pos = track_len.checked_add(2)?;
    // Matroska SimpleBlock flags: bit 7 is the keyframe flag.
    // Spec: https://www.matroska.org/technical/elements.html#SimpleBlock
    Some(block.get(flags_pos)? & 0x80 != 0)
}

fn collect_repaired_cue_points<'a>(data: &'a [u8], cues: Element, seekable_clusters: &HashSet<u64>) -> Option<Vec<&'a [u8]>> {
    let mut points = Vec::new();
    let mut pos = cues.data_start;
    while pos < cues.data_end {
        let point = read_element(data, pos)?;
        if point.id == ID_CUE_POINT && cue_point_targets_seekable_cluster(data, point, seekable_clusters)? {
            points.push(&data[point.start..point.end()]);
        }
        pos = point.end();
    }
    Some(points)
}

fn cue_point_targets_seekable_cluster(data: &[u8], point: Element, seekable_clusters: &HashSet<u64>) -> Option<bool> {
    let mut pos = point.data_start;
    while pos < point.data_end {
        let element = read_element(data, pos)?;
        if element.id == ID_CUE_TRACK_POSITIONS {
            if let Some(cluster_position) = cue_cluster_position(data, element) {
                return Some(seekable_clusters.contains(&cluster_position));
            }
        }
        pos = element.end();
    }
    Some(false)
}

fn cue_cluster_position(data: &[u8], positions: Element) -> Option<u64> {
    let mut pos = positions.data_start;
    while pos < positions.data_end {
        let element = read_element(data, pos)?;
        if element.id == ID_CUE_CLUSTER_POSITION {
            return read_uint(&data[element.data_start..element.data_end]);
        }
        pos = element.end();
    }
    None
}

fn find_element(data: &[u8], start: usize, end: usize, id: u64) -> Option<Element> {
    let mut pos = start;
    while pos < end {
        let element = read_element(data, pos)?;
        if element.id == id {
            return Some(element);
        }
        pos = element.end();
    }
    None
}

fn read_element(data: &[u8], pos: usize) -> Option<Element> {
    let (id, id_len) = read_id(data, pos)?;
    let size_pos = pos.checked_add(id_len)?;
    let (size, size_len) = read_vint(data, size_pos)?;
    let data_start = size_pos.checked_add(size_len)?;
    let data_end = data_start.checked_add(size as usize)?;
    if data_end > data.len() {
        return None;
    }
    Some(Element {
        id,
        start: pos,
        data_start,
        data_end,
        size_len,
    })
}

fn read_id(data: &[u8], pos: usize) -> Option<(u64, usize)> {
    let first = *data.get(pos)?;
    let len = vint_len(first)?;
    let mut value = 0u64;
    for i in 0..len {
        value = (value << 8) | (*data.get(pos + i)? as u64);
    }
    Some((value, len))
}

fn read_vint(data: &[u8], pos: usize) -> Option<(u64, usize)> {
    let first = *data.get(pos)?;
    let len = vint_len(first)?;
    let marker = 1u8 << (8 - len);
    let mut value = (first & !marker) as u64;
    for i in 1..len {
        value = (value << 8) | (*data.get(pos + i)? as u64);
    }
    Some((value, len))
}

fn vint_len(first: u8) -> Option<usize> {
    for len in 1..=8 {
        if first & (1 << (8 - len)) != 0 {
            return Some(len);
        }
    }
    None
}

fn read_uint(data: &[u8]) -> Option<u64> {
    let mut value = 0u64;
    for byte in data {
        value = (value << 8) | (*byte as u64);
    }
    Some(value)
}

fn write_vint_size(value: usize, len: usize, out: &mut Vec<u8>) -> Option<()> {
    if len == 0 || len > 8 {
        return None;
    }
    let max = (1usize << (7 * len)) - 2;
    if value > max {
        return None;
    }
    for i in (0..len).rev() {
        let mut byte = ((value >> (i * 8)) & 0xff) as u8;
        if i == len - 1 {
            byte |= 1 << (8 - len);
        }
        out.push(byte);
    }
    Some(())
}

fn void_element(total_len: usize) -> Option<Vec<u8>> {
    if total_len == 0 {
        return Some(Vec::new());
    }

    for size_len in 1..=8 {
        if total_len < 1 + size_len {
            continue;
        }

        let payload_len = total_len - 1 - size_len;
        let mut out = Vec::with_capacity(total_len);
        out.push(ID_VOID as u8);
        if write_vint_size(payload_len, size_len, &mut out).is_none() {
            continue;
        }
        out.resize(total_len, 0);
        return Some(out);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::convert::codec::{CodecWriter, VpxWriter};
    use media_server_protocol::media::{MediaMeta, MediaPacket};
    use std::io::Cursor;
    use webm::mux::{Segment, Track, VideoCodecId, Writer};

    #[test]
    fn repair_removes_raw_libwebm_cues_that_target_non_keyframe_clusters() {
        let data = raw_libwebm_with_non_keyframe_cluster_cues();
        let segment = find_element(&data, 0, data.len(), ID_SEGMENT).expect("segment");
        let cues = find_element(&data, segment.data_start, segment.data_end, ID_CUES).expect("cues");
        let cue_targets = cue_cluster_positions(&data, cues);
        let seekable_clusters = seekable_cluster_positions(&data, segment).expect("seekable cluster positions");

        assert_ne!(cue_targets, seekable_clusters, "raw libwebm cues should include non-keyframe cluster targets");
        assert!(cue_targets.len() > seekable_clusters.len(), "fixture should reproduce extra non-keyframe cues");

        let repaired = repair_cues_bytes(&data).expect("cues should be repaired");
        let repaired_segment = find_element(&repaired, 0, repaired.len(), ID_SEGMENT).expect("repaired segment");
        let repaired_cues = find_element(&repaired, repaired_segment.data_start, repaired_segment.data_end, ID_CUES).expect("repaired cues");

        assert_eq!(
            cue_cluster_positions(&repaired, repaired_cues),
            seekable_cluster_positions(&repaired, repaired_segment).expect("repaired seekable cluster positions")
        );
    }

    #[test]
    fn vpx_writer_finalizes_with_repaired_cues() {
        let mut output = Cursor::new(Vec::new());
        {
            let mut writer = VpxWriter::new(&mut output, 0);
            writer.push_media(0, vp8_packet(true));
            for ts in 1_000..100_000 {
                writer.push_media(ts, vp8_packet(false));
            }
            writer.push_media(100_000, vp8_packet(true));
        }

        let data = output.into_inner();
        let segment = find_element(&data, 0, data.len(), ID_SEGMENT).expect("segment");
        let cues = find_element(&data, segment.data_start, segment.data_end, ID_CUES).expect("cues");
        let cue_targets = cue_cluster_positions(&data, cues);
        let seekable_clusters = seekable_cluster_positions(&data, segment).expect("seekable cluster positions");

        assert!(cluster_count(&data, segment) > seekable_clusters.len(), "fixture should contain non-keyframe clusters");
        assert_eq!(cue_targets, seekable_clusters);
    }

    fn raw_libwebm_with_non_keyframe_cluster_cues() -> Vec<u8> {
        let mut output = Cursor::new(Vec::new());
        {
            let mut segment = Segment::new(Writer::new(&mut output)).expect("segment should be created");
            let mut video = segment.add_video_track(100, 100, None, VideoCodecId::VP8);
            video.add_frame(&[0], 0, true);
            for ts in 1_000..100_000 {
                video.add_frame(&[1], ts * 1_000_000, false);
            }
            video.add_frame(&[0], 100_000_000_000, true);
            segment.finalize(Some(101_000));
        }
        output.into_inner()
    }

    fn cluster_count(data: &[u8], segment: Element) -> usize {
        let mut count = 0;
        let mut pos = segment.data_start;
        while pos < segment.data_end {
            let element = read_element(data, pos).expect("element");
            if element.id == ID_CLUSTER {
                count += 1;
            }
            pos = element.end();
        }
        count
    }

    fn cue_cluster_positions(data: &[u8], cues: Element) -> HashSet<u64> {
        let mut positions = HashSet::new();
        let mut pos = cues.data_start;
        while pos < cues.data_end {
            let point = read_element(data, pos).expect("cue point");
            let mut inner = point.data_start;
            while inner < point.data_end {
                let element = read_element(data, inner).expect("cue child");
                if element.id == ID_CUE_TRACK_POSITIONS {
                    positions.insert(cue_cluster_position(data, element).expect("cue cluster position"));
                }
                inner = element.end();
            }
            pos = point.end();
        }
        positions
    }

    fn vp8_packet(key: bool) -> MediaPacket {
        MediaPacket {
            ts: 0,
            seq: 0,
            marker: true,
            nackable: false,
            layers: None,
            meta: MediaMeta::Vp8 { key, sim: None, rotation: None },
            data: vec![
                0x10,
                if key {
                    0
                } else {
                    1
                },
                0,
                0,
                0,
            ],
        }
    }
}
