use media_server_protocol::media::{MediaMeta, MediaOrientation, Vp9Profile, Vp9Svc};

use super::bit_read::BitRead;

const MAX_SPATIAL_LAYERS: u8 = 5;
const MAX_VP9REF_PICS: usize = 3;

pub fn parse_rtp(payload: &[u8], profile: Vp9Profile, rotation: Option<MediaOrientation>) -> Option<MediaMeta> {
    let mut header = Vp9Header::default();
    let (key, svc) = header.parse_from(payload).ok()?;
    Some(MediaMeta::Vp9 { key, profile, svc, rotation })
}

#[allow(unused_assignments)]
pub fn rewrite_rtp(payload: &mut [u8], svc: &Vp9Svc) {
    let mut payload_index = 0;

    let b = payload[payload_index];
    payload_index += 1;

    let i = (b & 0x80) != 0;

    // has PictureID
    if i {
        if payload[payload_index] & 0x80 > 0 {
            // M == 1, PID is 16bit
            payload[payload_index] = 0x80 | (svc.picture_id.unwrap_or(0) >> 8) as u8;
            payload[payload_index + 1] = svc.picture_id.unwrap_or(0) as u8;
            payload_index += 2;
        } else {
            //8bit
            payload[payload_index] = 0x7F & svc.picture_id.unwrap_or(0) as u8;
            payload_index += 1;
        }
    }
}

#[allow(unused, clippy::enum_variant_names)]
enum PacketError {
    ErrShortPacket,
    ErrTooManySpatialLayers,
    ErrTooManyPDiff,
}

#[derive(Default)]
struct Vp9Header {
    /// picture ID is present
    pub i: bool,
    /// inter-picture predicted frame.
    pub p: bool,
    /// layer indices present
    pub l: bool,
    /// flexible mode
    pub f: bool,
    /// start of frame. beginning of new vp9 frame
    pub b: bool,
    /// end of frame
    pub e: bool,
    /// scalability structure (SS) present
    pub v: bool,
    /// Not a reference frame for upper spatial layers
    pub z: bool,

    /// Recommended headers
    /// 7 or 16 bits, picture ID.
    pub picture_id: u16,

    /// Conditionally recommended headers
    /// Temporal layer ID
    pub tid: u8,
    /// Switching up point
    pub u: bool,
    /// Spatial layer ID
    pub sid: u8,
    /// Inter-layer dependency used
    pub d: bool,

    /// Conditionally required headers
    /// Reference index (F=1)
    pub pdiff: Vec<u8>,
    /// Temporal layer zero index (F=0)
    pub tl0picidx: u8,

    /// Scalability structure headers
    /// N_S + 1 indicates the number of spatial layers present in the VP9 stream
    pub ns: u8,
    /// Each spatial layer's frame resolution present
    pub y: bool,
    /// PG description present flag.
    pub g: bool,
    /// N_G indicates the number of pictures in a Picture Group (PG)
    pub ng: u8,
    /// Width
    pub width: Vec<u16>,
    /// Height
    pub height: Vec<u16>,
    /// Temporal layer ID of pictures in a Picture Group
    pub pgtid: Vec<u8>,
    /// Switching up point of pictures in a Picture Group
    pub pgu: Vec<bool>,
    /// Reference indices of pictures in a Picture Group
    pub pgpdiff: Vec<Vec<u8>>,
}

impl Vp9Header {
    #[allow(unused_assignments)]
    pub fn parse_from(&mut self, packet: &[u8]) -> Result<(bool, Option<Vp9Svc>), PacketError> {
        if packet.is_empty() {
            return Err(PacketError::ErrShortPacket);
        }

        let mut reader = (packet, 0);
        let b = reader.get_u8();

        self.i = (b & 0x80) != 0;
        self.p = (b & 0x40) != 0;
        self.l = (b & 0x20) != 0;
        self.f = (b & 0x10) != 0;
        self.b = (b & 0x08) != 0;
        self.e = (b & 0x04) != 0;
        self.v = (b & 0x02) != 0;
        self.z = (b & 0x01) != 0;

        let mut payload_index = 1;

        if self.i {
            payload_index = self.parse_picture_id(&mut reader, payload_index)?;
        }

        if self.l {
            payload_index = self.parse_layer_info(&mut reader, payload_index)?;
        }

        if self.f && self.p {
            payload_index = self.parse_ref_indices(&mut reader, payload_index)?;
        }

        if self.v {
            payload_index = self.parse_ssdata(&mut reader, payload_index)?;
        }

        let is_key = !self.p && self.b && (self.sid == 0 || !self.d);
        if self.l {
            Ok((
                is_key,
                Some(Vp9Svc {
                    picture_id: if self.i {
                        Some(self.picture_id)
                    } else {
                        None
                    },
                    spatial_layers: if self.v {
                        Some(self.ns + 1)
                    } else {
                        None
                    },
                    spatial: self.sid,
                    temporal: self.tid,
                    begin_frame: self.b,
                    end_frame: self.e,
                    switching_point: self.u,
                    predicted_frame: self.p,
                }),
            ))
        } else {
            Ok((is_key, None))
        }
    }

    // Picture ID:
    //
    //      +-+-+-+-+-+-+-+-+
    // I:   |M| PICTURE ID  |   M:0 => picture id is 7 bits.
    //      +-+-+-+-+-+-+-+-+   M:1 => picture id is 15 bits.
    // M:   | EXTENDED PID  |
    //      +-+-+-+-+-+-+-+-+
    //
    fn parse_picture_id(&mut self, reader: &mut dyn BitRead, mut payload_index: usize) -> Result<usize, PacketError> {
        if reader.remaining() == 0 {
            return Err(PacketError::ErrShortPacket);
        }
        let b = reader.get_u8();
        payload_index += 1;
        // PID present?
        if (b & 0x80) != 0 {
            if reader.remaining() == 0 {
                return Err(PacketError::ErrShortPacket);
            }
            // M == 1, PID is 15bit
            self.picture_id = (((b & 0x7f) as u16) << 8) | (reader.get_u8() as u16);
            payload_index += 1;
        } else {
            self.picture_id = (b & 0x7F) as u16;
        }

        Ok(payload_index)
    }

    fn parse_layer_info(&mut self, reader: &mut dyn BitRead, mut payload_index: usize) -> Result<usize, PacketError> {
        payload_index = self.parse_layer_info_common(reader, payload_index)?;

        if self.f {
            Ok(payload_index)
        } else {
            self.parse_layer_info_non_flexible_mode(reader, payload_index)
        }
    }

    // Layer indices (flexible mode):
    //
    //      +-+-+-+-+-+-+-+-+
    // L:   |  T  |U|  S  |D|
    //      +-+-+-+-+-+-+-+-+
    //
    fn parse_layer_info_common(&mut self, reader: &mut dyn BitRead, mut payload_index: usize) -> Result<usize, PacketError> {
        if reader.remaining() == 0 {
            return Err(PacketError::ErrShortPacket);
        }
        let b = reader.get_u8();
        payload_index += 1;

        self.tid = b >> 5;
        self.u = b & 0x10 != 0;
        self.sid = (b >> 1) & 0x7;
        self.d = b & 0x01 != 0;

        if self.sid >= MAX_SPATIAL_LAYERS {
            Err(PacketError::ErrTooManySpatialLayers)
        } else {
            Ok(payload_index)
        }
    }

    // Layer indices (non-flexible mode):
    //
    //      +-+-+-+-+-+-+-+-+
    // L:   |  T  |U|  S  |D|
    //      +-+-+-+-+-+-+-+-+
    //      |   tl0picidx   |
    //      +-+-+-+-+-+-+-+-+
    //
    fn parse_layer_info_non_flexible_mode(&mut self, reader: &mut dyn BitRead, mut payload_index: usize) -> Result<usize, PacketError> {
        if reader.remaining() == 0 {
            return Err(PacketError::ErrShortPacket);
        }
        self.tl0picidx = reader.get_u8();
        payload_index += 1;
        Ok(payload_index)
    }

    // Reference indices:
    //
    //      +-+-+-+-+-+-+-+-+                P=1,F=1: At least one reference index
    // P,F: | P_DIFF      |N|  up to 3 times          has to be specified.
    //      +-+-+-+-+-+-+-+-+                    N=1: An additional P_DIFF follows
    //                                                current P_DIFF.
    //
    fn parse_ref_indices(&mut self, reader: &mut dyn BitRead, mut payload_index: usize) -> Result<usize, PacketError> {
        let mut b = 1u8;
        while (b & 0x1) != 0 {
            if reader.remaining() == 0 {
                return Err(PacketError::ErrShortPacket);
            }
            b = reader.get_u8();
            payload_index += 1;

            self.pdiff.push(b >> 1);
            if self.pdiff.len() >= MAX_VP9REF_PICS {
                return Err(PacketError::ErrTooManyPDiff);
            }
        }

        Ok(payload_index)
    }

    // Scalability structure (SS):
    //
    //      +-+-+-+-+-+-+-+-+
    // V:   | N_S |Y|G|-|-|-|
    //      +-+-+-+-+-+-+-+-+              -|
    // Y:   |     WIDTH     | (OPTIONAL)    .
    //      +               +               .
    //      |               | (OPTIONAL)    .
    //      +-+-+-+-+-+-+-+-+               . N_S + 1 times
    //      |     HEIGHT    | (OPTIONAL)    .
    //      +               +               .
    //      |               | (OPTIONAL)    .
    //      +-+-+-+-+-+-+-+-+              -|
    // G:   |      N_G      | (OPTIONAL)
    //      +-+-+-+-+-+-+-+-+                           -|
    // N_G: |  T  |U| R |-|-| (OPTIONAL)                 .
    //      +-+-+-+-+-+-+-+-+              -|            . N_G times
    //      |    P_DIFF     | (OPTIONAL)    . R times    .
    //      +-+-+-+-+-+-+-+-+              -|           -|
    //
    fn parse_ssdata(&mut self, reader: &mut dyn BitRead, mut payload_index: usize) -> Result<usize, PacketError> {
        if reader.remaining() == 0 {
            return Err(PacketError::ErrShortPacket);
        }

        let b = reader.get_u8();
        payload_index += 1;

        self.ns = b >> 5;
        self.y = b & 0x10 != 0;
        self.g = (b >> 1) & 0x7 != 0;

        let ns = (self.ns + 1) as usize;
        self.ng = 0;

        if self.y {
            if reader.remaining() < 4 * ns {
                return Err(PacketError::ErrShortPacket);
            }

            self.width = vec![0u16; ns];
            self.height = vec![0u16; ns];
            for i in 0..ns {
                self.width[i] = reader.get_u16();
                self.height[i] = reader.get_u16();
            }
            payload_index += 4 * ns;
        }

        if self.g {
            if reader.remaining() == 0 {
                return Err(PacketError::ErrShortPacket);
            }

            self.ng = reader.get_u8();
            payload_index += 1;
        }

        for i in 0..self.ng as usize {
            if reader.remaining() == 0 {
                return Err(PacketError::ErrShortPacket);
            }
            let b = reader.get_u8();
            payload_index += 1;

            self.pgtid.push(b >> 5);
            self.pgu.push(b & 0x10 != 0);

            let r = ((b >> 2) & 0x3) as usize;
            if reader.remaining() < r {
                return Err(PacketError::ErrShortPacket);
            }

            self.pgpdiff.push(vec![]);
            for _ in 0..r {
                let b = reader.get_u8();
                payload_index += 1;

                self.pgpdiff[i].push(b);
            }
        }

        Ok(payload_index)
    }
}
