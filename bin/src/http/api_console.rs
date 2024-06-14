use tokio::sync::mpsc::Sender;

pub mod app;
pub mod cluster;
pub mod user;

#[derive(Clone)]
pub struct ConsoleApisCtx {
    pub sender: Sender<()>,
}
