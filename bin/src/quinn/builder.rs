use quinn::{ClientConfig, Endpoint, EndpointConfig, ServerConfig, TokioRuntime, TransportConfig};
use rustls::pki_types::{CertificateDer, PrivatePkcs8KeyDer};
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use super::vsocket::VirtualUdpSocket;

pub fn make_quinn_server(socket: VirtualUdpSocket, priv_key: PrivatePkcs8KeyDer<'static>, cert: CertificateDer<'static>) -> Result<Endpoint, Box<dyn Error>> {
    let server_config = configure_server(priv_key, cert)?;
    let runtime = Arc::new(TokioRuntime);
    let mut config = EndpointConfig::default();
    config.max_udp_payload_size(1500).expect("Should config quinn server max_size to 1500");
    Endpoint::new_with_abstract_socket(config, Some(server_config), Arc::new(socket), runtime).map_err(|e| e.into())
}

pub fn make_quinn_client(socket: VirtualUdpSocket, server_certs: &[CertificateDer]) -> Result<Endpoint, Box<dyn Error>> {
    let runtime = Arc::new(TokioRuntime);
    let mut config = EndpointConfig::default();
    //Note that client mtu size shoud be smaller than server's
    config.max_udp_payload_size(1400).expect("Should config quinn client max_size to 1400");
    let mut endpoint = Endpoint::new_with_abstract_socket(config, None, Arc::new(socket), runtime)?;
    endpoint.set_default_client_config(configure_client(server_certs)?);
    Ok(endpoint)
}

/// Returns default server configuration along with its certificate.
fn configure_server(priv_key: PrivatePkcs8KeyDer<'static>, cert: CertificateDer<'static>) -> Result<ServerConfig, Box<dyn Error>> {
    let cert_chain = vec![cert];

    let mut server_config = ServerConfig::with_single_cert(cert_chain, priv_key.into())?;
    let transport_config = Arc::get_mut(&mut server_config.transport).unwrap();
    transport_config.max_concurrent_uni_streams(0_u8.into());

    Ok(server_config)
}

fn configure_client(server_certs: &[CertificateDer]) -> Result<ClientConfig, Box<dyn Error>> {
    let mut certs = rustls::RootCertStore::empty();
    for cert in server_certs {
        certs.add(cert.clone())?;
    }
    let mut config = ClientConfig::with_root_certificates(Arc::new(certs))?;

    let mut transport = TransportConfig::default();
    transport.keep_alive_interval(Some(Duration::from_secs(3)));
    config.transport_config(Arc::new(transport));
    Ok(config)
}
