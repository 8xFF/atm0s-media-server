pub mod nats;

pub trait Transporter {
    fn send(&self, data: &[u8]) -> Result<(), String>;
    fn close(&mut self);
}
