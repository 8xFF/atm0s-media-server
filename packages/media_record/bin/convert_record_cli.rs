use clap::Parser;
use media_server_record::convert::{RecordConverter, RecordConverterOutput};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Record file converter for atm0s-media-server.
/// This tool allow convert room raw record to multiple webm files.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// S3 Source
    #[arg(env, long)]
    in_s3: String,

    /// S3 Dest
    #[arg(env, long)]
    out_s3: Option<String>,

    /// Folder Dest
    #[arg(env, long)]
    out_path: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), String> {
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "info");
    }
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        std::env::set_var("RUST_BACKTRACE", "1");
    }
    let args: Args = Args::parse();
    tracing_subscriber::registry().with(fmt::layer()).with(EnvFilter::from_default_env()).init();
    let output = if let Some(out_path) = args.out_s3 {
        RecordConverterOutput::S3(out_path)
    } else if let Some(out_path) = args.out_path {
        RecordConverterOutput::Local(out_path)
    } else {
        panic!("No output path or s3 uri");
    };
    let convert = RecordConverter::new(args.in_s3, output);
    let summary = convert.convert().await?;
    println!("{:?}", summary);
    Ok(())
}
