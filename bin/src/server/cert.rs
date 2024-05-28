use std::time::{SystemTime, UNIX_EPOCH};

use clap::Parser;

/// A Certs util for quic, which generate der cert and key based on domain
#[derive(Debug, Parser)]
pub struct Args {
    /// Domains
    #[arg(env, long)]
    domains: Vec<String>,
}

pub async fn run_cert_utils(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let cert = rcgen::generate_simple_self_signed(args.domains)?;
    let start = SystemTime::now();
    let since_the_epoch = start.duration_since(UNIX_EPOCH).expect("Time went backwards").as_millis();
    std::fs::write(format!("./certificate-{}.cert", since_the_epoch), cert.cert.der())?;
    std::fs::write(format!("./certificate-{}.key", since_the_epoch), cert.key_pair.serialize_der())?;
    Ok(())
}
