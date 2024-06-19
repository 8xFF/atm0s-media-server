use clap::Subcommand;

#[cfg(feature = "cert_utils")]
mod cert;
#[cfg(feature = "connector")]
mod connector;
#[cfg(feature = "console")]
mod console;
#[cfg(feature = "gateway")]
mod gateway;
#[cfg(feature = "media")]
mod media;

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
}
