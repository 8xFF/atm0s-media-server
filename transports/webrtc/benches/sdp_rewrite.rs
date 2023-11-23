use atm0s_media_server_transport_webrtc::{SdpBoxRewriteScope, SdpBox};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};

fn criterion_benchmark(c: &mut Criterion) {
    let mut sdp_rewrite = SdpBox {
        scope: SdpBoxRewriteScope::StreamAndTrack,
    };
    let sdp_answer = include_str!("./sample.sdp");

    let mut group = c.benchmark_group("sdp_rewrite");
    group.throughput(criterion::Throughput::Bytes(sdp_answer.len() as u64));

    group.bench_function(BenchmarkId::new("rewrite_sdp", sdp_answer.len()), |b| b.iter(|| sdp_rewrite.rewrite_answer(sdp_answer)));

    group.finish();

    let mut group = c.benchmark_group("sdp_rewrite");
    group.throughput(criterion::Throughput::Elements(1));

    group.bench_function(BenchmarkId::new("rewrite_sdp", sdp_answer.len()), |b| b.iter(|| sdp_rewrite.rewrite_answer(sdp_answer)));

    group.finish();
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
