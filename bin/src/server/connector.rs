use clap::Parser;

#[derive(Debug, Parser)]
pub struct Args {}

pub async fn run_media_connector(args: Args) {
    println!("Running media connector");
}
