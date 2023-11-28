use super::bit_read::BitRead;
use transport::Vp8Simulcast;

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

impl Vp8Header {
    pub fn parse_from(&mut self, packet: &[u8], rid: Option<u16>) -> (bool, Option<Vp8Simulcast>) {
        let payload_len = packet.len();
        if payload_len < 4 {
            return (false, None);
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

        let mut reader = (packet, 0);
        let mut payload_index = 0;

        let mut b = reader.get_u8();
        payload_index += 1;

        self.x = (b & 0x80) >> 7;
        self.n = (b & 0x20) >> 5;
        self.s = (b & 0x10) >> 4;
        self.pid = b & 0x07;

        if self.x == 1 {
            b = reader.get_u8();
            payload_index += 1;
            self.i = (b & 0x80) >> 7;
            self.l = (b & 0x40) >> 6;
            self.t = (b & 0x20) >> 5;
            self.k = (b & 0x10) >> 4;
        }

        if self.i == 1 {
            b = reader.get_u8();
            payload_index += 1;
            // PID present?
            if b & 0x80 > 0 {
                // M == 1, PID is 16bit
                self.picture_id = (((b & 0x7f) as u16) << 8) | (reader.get_u8() as u16);
                payload_index += 1;
            } else {
                self.picture_id = b as u16;
            }
        }

        if payload_index >= payload_len {
            return (false, None);
        }

        if self.l == 1 {
            self.tl0_pic_idx = reader.get_u8();
            payload_index += 1;
        }

        if payload_index >= payload_len {
            return (false, None);
        }

        if self.t == 1 || self.k == 1 {
            let b = reader.get_u8();
            if self.t == 1 {
                self.tid = b >> 6;
                self.y = (b >> 5) & 0x1;
            }
            if self.k == 1 {
                self.key_idx = b & 0x1F;
            }
            payload_index += 1;
        }

        if payload_index >= packet.len() {
            return (false, None);
        }

        let out = &packet[payload_index..];

        let is_key = self.s != 0 && self.pid == 0 && out[0] & 0x01 == 0;
        if self.t == 1 {
            (
                is_key,
                Some(Vp8Simulcast {
                    spatial: rid.unwrap_or(0) as u8,
                    temporal: self.tid,
                    picture_id: if self.i != 0 {
                        Some(self.picture_id)
                    } else {
                        None
                    },
                    tl0_pic_idx: if self.l != 0 {
                        Some(self.tl0_pic_idx)
                    } else {
                        None
                    },
                    layer_sync: self.y != 0,
                }),
            )
        } else {
            (is_key, None)
        }
    }
}

pub fn payload_parse(payload: &[u8], rid: Option<u16>) -> (bool, Option<Vp8Simulcast>) {
    let mut vp8_header = Vp8Header::default();
    vp8_header.parse_from(payload, rid)
}

#[allow(unused_assignments)]
pub fn payload_rewrite(payload: &mut [u8], codec: &Vp8Simulcast) {
    let mut payload_index = 0;

    let b = payload[payload_index];
    payload_index += 1;

    let x = (b & 0x80) >> 7;
    let mut i = 0;
    let mut l = 0;
    if x == 1 {
        let b = payload[payload_index];
        payload_index += 1;
        i = (b & 0x80) >> 7;
        l = (b & 0x40) >> 6;
    }

    // has PictureID
    if i == 1 {
        if payload[payload_index] & 0x80 > 0 {
            // M == 1, PID is 16bit
            payload[payload_index] = 0x80 | (codec.picture_id.unwrap_or(0) >> 8) as u8;
            payload[payload_index + 1] = codec.picture_id.unwrap_or(0) as u8;
            payload_index += 2;
        } else {
            //8bit
            payload[payload_index] = 0x7F & codec.picture_id.unwrap_or(0) as u8;
            payload_index += 1;
        }
    }

    if l == 1 {
        payload[payload_index] = codec.tl0_pic_idx.unwrap_or(0);
        payload_index += 1;
    }
}

//TODO test this
