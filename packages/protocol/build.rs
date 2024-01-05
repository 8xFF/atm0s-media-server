extern crate prost_build;

fn main() {
    let mut config = prost_build::Config::new();
    config.compile_protos(&["src/atm0s.proto"], &["src/"]).unwrap();
}
