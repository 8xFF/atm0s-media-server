use media_server_record::{ComposeSessionWebm, SessionReader};
use rusty_s3::{Bucket, Credentials, UrlStyle};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

#[tokio::main]
async fn main() {
    log::info!("start");
    tracing_subscriber::registry().with(fmt::layer()).with(EnvFilter::from_default_env()).init();
    let s3 = Bucket::new("http://localhost:9000".parse().unwrap(), UrlStyle::Path, "record", "").unwrap();
    let mut session_reader = SessionReader::new(s3, Credentials::new("minioadmin", "minioadmin"), "demo/user1/3291916399382158078").await.unwrap();
    let mut compose = ComposeSessionWebm::new();
    while let Some(event) = session_reader.recv().await {
        compose.push(event);
    }
}
