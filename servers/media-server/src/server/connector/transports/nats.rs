use std::{io, marker::PhantomData, time::Duration};

use async_std::stream::{interval, Interval, StreamExt};
use async_trait::async_trait;
use futures::{select, FutureExt};
use prost::Message;
use yaque::Receiver;

use super::ConnectorTransporter;

pub struct NatsTransporter<M: Message + Clone> {
    conn: nats::asynk::Connection,
    subject: String,
    rx: Receiver,
    tick: Interval,
    _tmp: PhantomData<M>,
}

impl<M: Message + Clone> NatsTransporter<M> {
    pub async fn new(uri: String, subject: String, rx: Receiver) -> Result<Self, io::Error> {
        log::info!("Connecting to NATS server: {}", uri);
        Ok(Self {
            rx,
            conn: nats::asynk::Options::new()
                .retry_on_failed_connect()
                .max_reconnects(999999999) //big value ensure nats will auto reconnect forever in theory
                .reconnect_delay_callback(|c| Duration::from_millis(std::cmp::min((c * 100) as u64, 10000))) //max wait 10s
                .disconnect_callback(|| log::warn!("connection has been lost"))
                .reconnect_callback(|| log::warn!("connection has been reestablished"))
                .close_callback(|| panic!("connection has been closed")) //this should not happend
                .connect(uri)
                .await?,
            subject,
            tick: interval(Duration::from_millis(1000)),
            _tmp: Default::default(),
        })
    }

    async fn awake(&mut self) {
        while let Ok(queue_data) = self.rx.try_recv() {
            if let Err(e) = self.conn.publish(&self.subject, queue_data.clone()).await {
                log::error!("Error sending message: {:?}, saving it into memory for later", e);
                break;
            } else {
                queue_data.commit();
            }
        }
    }
}

#[async_trait]
impl<M: Message + Clone> ConnectorTransporter<M> for NatsTransporter<M> {
    async fn poll(&mut self) -> Result<(), io::Error> {
        log::debug!("Polling Nats transporter");
        select! {
            _ = self.tick.next().fuse() => {}
            e = self.rx.recv().fuse() => match e {
                Ok(data) => {
                    log::debug!("Sending data to nats");
                    if let Ok(_) = self.conn.publish(&self.subject, data.clone()).await {
                        let _ = data.commit();
                    } else {
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

    async fn close(&mut self) -> Result<(), io::Error> {
        self.conn.close().await?;
        Ok(())
    }
}
