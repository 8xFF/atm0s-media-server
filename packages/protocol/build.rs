#[cfg(feature = "build-protobuf")]
use prost_build::{Config, ServiceGenerator};
#[cfg(feature = "build-protobuf")]
use std::fmt::Write;
use std::io::Result;
#[cfg(feature = "build-protobuf")]
use tera::{Context, Tera};

fn main() -> Result<()> {
    #[cfg(feature = "build-protobuf")]
    Config::new()
        .service_generator(Box::new(GenericRpcGenerator))
        .out_dir("src/protobuf")
        .include_file("mod.rs")
        .type_attribute("cluster_connector.PeerEvent.RouteBegin", "#[derive(serde::Serialize)]")
        .type_attribute("cluster_connector.PeerEvent.RouteSuccess", "#[derive(serde::Serialize)]")
        .type_attribute("cluster_connector.PeerEvent.RouteError", "#[derive(serde::Serialize)]")
        .type_attribute("cluster_connector.PeerEvent.Connecting", "#[derive(serde::Serialize)]")
        .type_attribute("cluster_connector.PeerEvent.ConnectError", "#[derive(serde::Serialize)]")
        .type_attribute("cluster_connector.PeerEvent.Connected", "#[derive(serde::Serialize)]")
        .type_attribute("cluster_connector.PeerEvent.Reconnecting", "#[derive(serde::Serialize)]")
        .type_attribute("cluster_connector.PeerEvent.Reconnected", "#[derive(serde::Serialize)]")
        .type_attribute("cluster_connector.PeerEvent.Disconnected", "#[derive(serde::Serialize)]")
        .type_attribute("cluster_connector.PeerEvent.Join", "#[derive(serde::Serialize)]")
        .type_attribute("cluster_connector.PeerEvent.Leave", "#[derive(serde::Serialize)]")
        .type_attribute("cluster_connector.PeerEvent.RemoteTrackStarted", "#[derive(serde::Serialize)]")
        .type_attribute("cluster_connector.PeerEvent.RemoteTrackEnded", "#[derive(serde::Serialize)]")
        .type_attribute("cluster_connector.PeerEvent.LocalTrackStarted", "#[derive(serde::Serialize)]")
        .type_attribute("cluster_connector.PeerEvent.LocalTrackAttach", "#[derive(serde::Serialize)]")
        .type_attribute("cluster_connector.PeerEvent.LocalTrackDetach", "#[derive(serde::Serialize)]")
        .type_attribute("cluster_connector.PeerEvent.LocalTrackEnded", "#[derive(serde::Serialize)]")
        .compile_protos(
            &[
                "./proto/shared.proto",
                "./proto/sdk/session.proto",
                "./proto/sdk/features.proto",
                "./proto/sdk/features.mixer.proto",
                "./proto/sdk/gateway.proto",
                "./proto/cluster/gateway.proto",
                "./proto/cluster/connector.proto",
            ],
            &["./proto"],
        )?;
    Ok(())
}

#[cfg(feature = "build-protobuf")]
struct GenericRpcGenerator;

#[cfg(feature = "build-protobuf")]
impl ServiceGenerator for GenericRpcGenerator {
    fn generate(&mut self, service: prost_build::Service, buf: &mut String) {
        #[derive(serde::Serialize, Debug)]
        struct MethodInfo {
            name: String,
            input: String,
            output: String,
        }

        let methods = service
            .methods
            .into_iter()
            .map(|m| MethodInfo {
                name: m.name,
                input: m.input_type,
                output: m.output_type,
            })
            .collect::<Vec<_>>();

        println!("{:?}", methods);

        let mut tera = Tera::default();
        tera.add_raw_template("service", include_str!("./build_templates/service.teml"))
            .expect("Should include service template");
        let mut context = Context::new();
        context.insert("service", &service.name);
        context.insert("methods", &methods);
        tera.render_to("service", &context, StringWriter(buf)).expect("Should success to write");
    }
}

#[cfg(feature = "build-protobuf")]
struct StringWriter<'a>(&'a mut String);

#[cfg(feature = "build-protobuf")]
impl<'a> std::io::Write for StringWriter<'a> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.0.write_str(String::from_utf8_lossy(buf).to_string().as_str()).expect("Should write ok");
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}
