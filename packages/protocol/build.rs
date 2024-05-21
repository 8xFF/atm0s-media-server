use std::io::Result;

#[cfg(any(feature = "build-protobuf"))]
use prost_build::Config;

fn main() -> Result<()> {
    #[cfg(feature = "build-protobuf")]
    Config::new()
        .out_dir("src/protobuf")
        .include_file("mod.rs")
        .compile_protos(&["./proto/shared.proto", "./proto/conn.proto", "./proto/features.proto", "./proto/gateway.proto"], &["./proto"])?;
    Ok(())
}
