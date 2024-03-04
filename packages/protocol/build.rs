extern crate prost_build;

fn main() {
    let mut config = prost_build::Config::new();
    config.type_attribute(".", "#[derive(serde::Serialize)]");
    config.compile_protos(&["src/atm0s.proto"], &["src/"]).unwrap();
}
