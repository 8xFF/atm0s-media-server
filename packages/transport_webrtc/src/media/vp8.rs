use super::bit_read::BitRead;
use media_server_protocol::media::{MediaMeta, Vp8Sim};

pub fn parse_rtp(packet: &[u8], rid: Option<u8>) -> Option<MediaMeta> {
    let payload_len = packet.len();
    if payload_len < 4 {
        return None;
    }

    let mut reader = (packet, 0);
    let mut payload_index = 0;

    let mut b = reader.get_u8();
    payload_index += 1;

    let mut vp8 = Vp8Header::default();

    vp8.x = (b & 0x80) >> 7;
    vp8.n = (b & 0x20) >> 5;
    vp8.s = (b & 0x10) >> 4;
    vp8.pid = b & 0x07;

    if vp8.x == 1 {
        b = reader.get_u8();
        payload_index += 1;
        vp8.i = (b & 0x80) >> 7;
        vp8.l = (b & 0x40) >> 6;
        vp8.t = (b & 0x20) >> 5;
        vp8.k = (b & 0x10) >> 4;
    }

    if vp8.i == 1 {
        b = reader.get_u8();
        payload_index += 1;
        // PID present?
        if b & 0x80 > 0 {
            // M == 1, PID is 16bit
            vp8.picture_id = (((b & 0x7f) as u16) << 8) | (reader.get_u8() as u16);
            payload_index += 1;
        } else {
            vp8.picture_id = b as u16;
        }
    }

    if payload_index >= payload_len {
        return None;
    }

    if vp8.l == 1 {
        vp8.tl0_pic_idx = reader.get_u8();
        payload_index += 1;
    }

    if payload_index >= payload_len {
        return None;
    }

    if vp8.t == 1 || vp8.k == 1 {
        let b = reader.get_u8();
        if vp8.t == 1 {
            vp8.tid = b >> 6;
            vp8.y = (b >> 5) & 0x1;
        }
        if vp8.k == 1 {
            vp8.key_idx = b & 0x1F;
        }
        payload_index += 1;
    }

    if payload_index >= packet.len() {
        return None;
    }

    let out = &packet[payload_index..];

    let is_key = vp8.s != 0 && vp8.pid == 0 && out[0] & 0x01 == 0;
    if vp8.t == 1 {
        Some(MediaMeta::Vp8 {
            key: is_key,
            sim: Some(Vp8Sim {
                spatial: rid.unwrap_or(0),
                temporal: vp8.tid,
                picture_id: if vp8.i != 0 {
                    Some(vp8.picture_id)
                } else {
                    None
                },
                tl0_pic_idx: if vp8.l != 0 {
                    Some(vp8.tl0_pic_idx)
                } else {
                    None
                },
                layer_sync: vp8.y != 0,
            }),
        })
    } else {
        Some(MediaMeta::Vp8 { key: is_key, sim: None })
    }
}

//    0 1 2 3 4 5 6 7                      0 1 2 3 4 5 6 7
//    +-+-+-+-+-+-+-+-+                   +-+-+-+-+-+-+-+-+
//    |X|R|N|S|R| PID | (REQUIRED)        |X|R|N|S|R| PID | (REQUIRED)
//    +-+-+-+-+-+-+-+-+                   +-+-+-+-+-+-+-+-+
// X: |I|L|T|K| RSV   | (OPTIONAL)   X:   |I|L|T|K| RSV   | (OPTIONAL)
//    +-+-+-+-+-+-+-+-+                   +-+-+-+-+-+-+-+-+
// I: |M| PictureID   | (OPTIONAL)   I:   |M| PictureID   | (OPTIONAL)
//    +-+-+-+-+-+-+-+-+                   +-+-+-+-+-+-+-+-+
// L: |   tl0picidx   | (OPTIONAL)        |   PictureID   |
//    +-+-+-+-+-+-+-+-+                   +-+-+-+-+-+-+-+-+
//T/K:|tid|Y| KEYIDX  | (OPTIONAL)   L:   |   tl0picidx   | (OPTIONAL)
//    +-+-+-+-+-+-+-+-+                   +-+-+-+-+-+-+-+-+
//T/K:|tid|Y| KEYIDX  | (OPTIONAL)
//    +-+-+-+-+-+-+-+-+
#[derive(Default)]
struct Vp8Header {
    /// Required Header
    /// extended controlbits present
    pub x: u8,
    /// when set to 1 this frame can be discarded
    pub n: u8,
    /// start of VP8 partition
    pub s: u8,
    /// partition index
    pub pid: u8,

    /// Extended control bits
    /// 1 if PictureID is present
    pub i: u8,
    /// 1 if tl0picidx is present
    pub l: u8,
    /// 1 if tid is present
    pub t: u8,
    /// 1 if KEYIDX is present
    pub k: u8,

    /// Optional extension
    /// 8 or 16 bits, picture ID
    pub picture_id: u16,

    /// 8 bits temporal level zero index
    pub tl0_pic_idx: u8,
    /// 2 bits temporal layer index
    pub tid: u8,
    /// 1 bit layer sync bit
    pub y: u8,
    /// 5 bits temporal key frame index
    pub key_idx: u8,
}
