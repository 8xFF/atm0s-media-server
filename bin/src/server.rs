use clap::Subcommand;

#[cfg(feature = "cert_utils")]
mod cert;
#[cfg(feature = "connector")]
pub mod connector;
#[cfg(feature = "console")]
pub mod console;
#[cfg(feature = "gateway")]
pub mod gateway;
#[cfg(feature = "media")]
pub mod media;
#[cfg(feature = "standalone")]
pub mod standalone;

#[cfg(feature = "cert_utils")]
pub use cert::run_cert_utils;
#[cfg(feature = "connector")]
pub use connector::run_media_connector;
#[cfg(feature = "console")]
pub use console::{run_console_server, storage as console_storage};
#[cfg(feature = "gateway")]
pub use gateway::run_media_gateway;
#[cfg(feature = "media")]
pub use media::run_media_server;
#[cfg(feature = "standalone")]
pub use standalone::run_standalone;

#[derive(Debug, Subcommand)]
pub enum ServerType {
    #[cfg(feature = "console")]
    Console(console::Args),
    #[cfg(feature = "gateway")]
    Gateway(gateway::Args),
    #[cfg(feature = "connector")]
    Connector(connector::Args),
    #[cfg(feature = "media")]
    Media(media::Args),
    #[cfg(feature = "cert_utils")]
    Cert(cert::Args),
    #[cfg(feature = "standalone")]
    Standalone(standalone::Args),
}
