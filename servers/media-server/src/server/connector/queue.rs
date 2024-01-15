use std::{io, time::Duration};

use async_std::stream::{interval, Interval, StreamExt};
use futures::{select, FutureExt};
use prost::Message;

use super::transports::ConnectorTransporter;

pub struct TransporterQueue<M> {
    tick: Interval,
    rx: yaque::Receiver,
    transporter: Box<dyn ConnectorTransporter<M>>,
}

impl<M: Message + Clone + TryFrom<Vec<u8>>> TransporterQueue<M> {
    pub fn new(base: &str, transporter: Box<dyn ConnectorTransporter<M>>) -> Result<(Self, yaque::Sender), io::Error> {
        let (tx, rx) = yaque::channel(base)?;
        Ok((
            Self {
                rx,
                transporter,
                tick: interval(Duration::from_millis(1000)),
            },
            tx,
        ))
    }

    async fn awake(&mut self) {
        while let Ok(queue_data) = self.rx.try_recv() {
            if let Ok(data) = queue_data.clone().try_into() {
                if let Err(e) = self.transporter.send(data).await {
                    log::error!("Error sending message: {:?}, saving it into memory for later", e);
                    break;
                } else {
                    let _ = queue_data.commit();
                }
            } else {
                log::error!("Error decoding message, saving it into memory for later");
                break;
            }
        }
    }

    pub async fn poll(&mut self) -> Result<(), io::Error> {
        log::debug!("Polling Nats transporter");
        select! {
            _ = self.tick.next().fuse() => {}
            e = self.rx.recv().fuse() => match e {
                Ok(data) => {
                    log::debug!("Sending data to nats");
                    if let Ok(send_data) = data.clone().try_into() {
                        if let Err(err) = self.transporter.send(send_data).await {
                            log::error!("Error sending message: {:?}, saving it into memory for later", err);
                            return Ok(());
                        } else {
                            let _ = data.commit();
                        }
                    } else {
                      log::error!("Error decoding message, saving it into memory for later");
                      return Ok(());
                    }
                },
                Err(err) => {
                    return Err(err);
                }
            }
        };

        self.awake().await;
        Ok(())
    }
}
