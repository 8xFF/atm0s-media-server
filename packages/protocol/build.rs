extern crate prost_build;

fn main() {
    let mut config = prost_build::Config::new();
    // config.type_attribute(".", "");
    // config.type_attribute(".receive_stream_event", "#[derive(Eq)]");
    // config.type_attribute(".send_stream_event", "#[derive(Eq)]");
    config.compile_protos(&["src/atm0s.proto"],
                                &["src/"]).unwrap();
}
