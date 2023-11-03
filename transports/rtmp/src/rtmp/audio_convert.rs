use std::collections::VecDeque;

use bytes::{Bytes, BytesMut};
use transport::MediaPacket;
use xflv::demuxer::FlvAudioTagDemuxer;

//TODO add audio resample with soxr or other lib
/*
let io_spec = IOSpec::new(Datatype::Int16I, Datatype::Int16I);
let quality_spec = QualitySpec::new(&QualityRecipe::VeryHigh, QualityFlags::HI_PREC_CLOCK);
let resample_441khz = Soxr::create(44100.0, 48000.0, 2, Some(&io_spec), Some(&quality_spec), None).unwrap();
let mut audio = vec![0i16; STEREO_48K_20MS];
if let Err(err) = self.resample_441khz.process(Some(&audio_441), &mut audio) {
    log::error!("resample 44.1khz to 48khz error {}", err);
    continue;
}
 */
pub struct RtmpAacToMediaPacketOpus {
    demuxer: FlvAudioTagDemuxer,
    decoder: fdk_aac::dec::Decoder,
    buffer: VecDeque<i16>,
    encoder: opus::Encoder,
    outputs: VecDeque<MediaPacket>,
    seq_no: u16,
    time: u32,
}

impl RtmpAacToMediaPacketOpus {
    pub fn new() -> Self {
        Self {
            demuxer: FlvAudioTagDemuxer::new(),
            decoder: fdk_aac::dec::Decoder::new(fdk_aac::dec::Transport::Adts),
            buffer: VecDeque::with_capacity(960 * 4),
            encoder: opus::Encoder::new(48000, opus::Channels::Stereo, opus::Application::Voip).unwrap(),
            outputs: VecDeque::new(),
            seq_no: 0,
            time: 0,
        }
    }

    pub fn push(&mut self, data: Bytes, ts_ms: u32) -> Option<()> {
        let data = BytesMut::from(&data as &[u8]);
        let frame = self.demuxer.demux(ts_ms, data).ok()?;
        self.decoder.fill(&frame.data).map(|_| ()).ok()?;
        let mut decode_frame = vec![0; 1024 * 2];
        self.decoder.decode_frame(&mut decode_frame).ok()?;

        let frame_size = self.decoder.decoded_frame_size();
        for i in 0..frame_size {
            self.buffer.push_back(decode_frame[i]);
        }

        let info = self.decoder.stream_info();
        log::debug!("on aac decoded frame size: {} info: {:?}", frame_size, info);

        while self.buffer.len() > 980 * 2 {
            //20ms * 2 channels
            let mut pcm = vec![0; 960 * 2];
            for i in 0..960 {
                pcm[i * 2] = self.buffer.pop_front().unwrap();
                pcm[i * 2 + 1] = self.buffer.pop_front().unwrap();
            }
            let mut encoded_opus = vec![0; 1500];
            if let Ok(len) = self.encoder.encode(&pcm, &mut encoded_opus) {
                unsafe {
                    encoded_opus.set_len(len);
                }
                self.outputs.push_back(MediaPacket::simple_audio(self.seq_no, self.time, encoded_opus));
                self.time += 960;
                self.seq_no += 1;
            }
        }

        Some(())
    }

    pub fn pop(&mut self) -> Option<MediaPacket> {
        self.outputs.pop_front()
    }
}
