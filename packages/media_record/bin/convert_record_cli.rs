use atm0s_media_server_record::convert::{RecordComposerConfig, RecordConvert, RecordConvertConfig, RecordConvertOutputLocation};
use clap::Parser;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Record file converter for atm0s-media-server.
/// This tool allow convert room raw record to multiple webm files.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// S3 Source
    #[arg(env, long)]
    in_s3: String,

    /// Transmux
    #[arg(env, long)]
    transmux: bool,

    /// Transmux S3 Dest
    #[arg(env, long)]
    transmux_out_s3: Option<String>,

    /// Transmux Folder Dest
    #[arg(env, long)]
    transmux_out_path: Option<String>,

    /// Compose audio
    #[arg(env, long)]
    compose_audio: bool,

    /// Compose video
    #[arg(env, long)]
    compose_video: bool,

    /// Compose S3 URL
    #[arg(env, long)]
    compose_out_s3: Option<String>,

    /// Compose File Path
    #[arg(env, long)]
    compose_out_path: Option<String>,
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
    let convert = RecordConvert::new(RecordConvertConfig {
        in_s3: args.in_s3,
        transmux: if let Some(out_path) = args.transmux_out_s3 {
            Some(RecordConvertOutputLocation::S3(out_path))
        } else {
            args.transmux_out_path.map(RecordConvertOutputLocation::Local)
        },
        compose: if args.compose_audio || args.compose_video {
            Some(RecordComposerConfig {
                audio: args.compose_audio,
                video: args.compose_video,
                output_relative: "".to_string(),
                output: if let Some(out_path) = args.compose_out_s3 {
                    RecordConvertOutputLocation::S3(out_path)
                } else if let Some(out_path) = args.compose_out_path {
                    RecordConvertOutputLocation::Local(out_path)
                } else {
                    panic!("No output path or s3 uri");
                },
            })
        } else {
            None
        },
    });
    let summary = convert.convert().await?;
    println!("{:?}", summary);
    Ok(())
}
