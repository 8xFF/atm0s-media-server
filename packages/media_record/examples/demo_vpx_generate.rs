use std::f32::consts::PI;

use atm0s_media_server_record::convert::{CodecWriter, VpxWriter};
use media_server_codecs::{opus::OpusEncoder, AudioEncodder};
use media_server_protocol::media::MediaPacket;

/// Generate a mono sine wave.
///
/// - `freq_hz`: frequency of the sine wave, e.g. 440.0
/// - `sample_rate`: samples per second, e.g. 48000
/// - `duration_secs`: length in seconds
pub fn generate_sine(freq_hz: f32, sample_rate: u32, duration_secs: f32) -> Vec<i16> {
    let total_samples = (sample_rate as f32 * duration_secs) as usize;
    let dt = 1.0 / sample_rate as f32;

    (0..total_samples)
        .map(|n| {
            let t = n as f32 * dt; // time in seconds
            ((2.0 * PI * freq_hz * t).sin() * 32767.0) as i16 // sin(2π f t)
        })
        .collect()
}

fn main() {
    let mut codec = OpusEncoder::default();
    let mut vpx_writer = VpxWriter::new(std::fs::File::create("test.webm").unwrap(), 0);

    let sine = generate_sine(480.0, 48000, 2.0);

    // generate sine wave
    let mut out_buf = vec![0; 1024];
    for (seq, frame) in sine.chunks(960).enumerate() {
        let out_len = codec.encode(frame, &mut out_buf).unwrap();
        println!("out_len: {}", out_len);
        let pkt = MediaPacket::build_audio(seq as u32, seq as u16, None, out_buf[..out_len].to_vec());
        vpx_writer.push_media((seq * 20) as u64, pkt);
    }
}
