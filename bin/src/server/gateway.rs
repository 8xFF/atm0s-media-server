use clap::Parser;

#[derive(Debug, Parser)]
pub struct Args {}

pub async fn run_media_gateway(args: Args) {
    println!("Running media gateway");
}
