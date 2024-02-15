# Simulcast

For implementing simulcast, each media packet will have a codec meta:

```rust
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize, Default)]
pub enum PayloadCodec {
    Vp8(bool, Option<Vp8Simulcast>),
    Vp9(bool, Vp9Profile, Option<Vp9Svc>),
    H264(bool, H264Profile, Option<H264Simulcast>),
    #[default]
    Opus,
}
```

Example with Vp8 we have:

```rust
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Vp8Simulcast {
    pub picture_id: Option<u16>,
    pub tl0_pic_idx: Option<u8>,
    pub spatial: u8,
    pub temporal: u8,
    pub layer_sync: bool,
}
```

In there, we use `spatial` and `temporal` to identify which layer of simulcast we are using and process upgrade/downgrade simulcast.

The logic of simulcast/svc is put on: endpoint/internal/local_track/scalable_filter.rs

Currently we supported:

- H264 Simulcast
- VP8 Simulcast
- VP9 SVC

For implementing more support with other codecs such as AV1, we need to add more types to PayloadCodec and implement the `ScalableFilter` trait for that codec.

```rust
trait ScalableFilter: Send + Sync {
    fn pause(&mut self);

    fn resume(&mut self);

    /// Configure the target layer to send to the remote peer. If return true => should send a key frame.
    fn set_target_layer(&mut self, spatial: u8, temporal: u8, key_only: bool) -> bool;

    /// Returns true if the packet should be sent to the remote peer.
    /// This is used to implement simulcast and SVC.
    /// The packet is modified in place to remove layers that should not be sent.
    /// Also return stream just changed or not, in case of just changed => need reinit seq and ts rewriter
    fn should_send(&mut self, pkt: &mut MediaPacket) -> (FilterResult, bool);
}
```
