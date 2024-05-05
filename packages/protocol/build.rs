use std::io::Result;

use prost_build::compile_protos;

fn main() -> Result<()> {
    compile_protos(&["./proto/shared.proto", "./proto/conn.proto", "./proto/features.proto", "./proto/gateway.proto"], &["./proto"])
}
