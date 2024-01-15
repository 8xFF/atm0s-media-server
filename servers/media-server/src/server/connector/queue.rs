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
        if let Err(err) = yaque::recovery::recover(base) {
            log::warn!("Error trying to recover queue, maybe first time: {:?}", err);
        }
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
#[cfg(test)]
mod tests {

    use async_std::channel::{self, Receiver, Sender};
    use protocol::media_event_logs::{
        media_endpoint_log_request,
        session_event::{self, SessionStats},
        MediaEndpointLogRequest, SessionEvent,
    };

    use super::*;
    #[async_std::test]
    async fn should_queue_and_send_successfully() {
        // Create a mock ConnectorTransporter
        struct MockTransporter<M> {
            tx: Sender<M>,
        }

        impl<M> MockTransporter<M> {
            pub fn new() -> (Self, Receiver<M>) {
                let (tx, rx) = channel::bounded(1);
                (Self { tx: tx.clone() }, rx.clone())
            }
        }

        #[async_trait::async_trait]
        impl<M: Message + Clone + TryFrom<Vec<u8>>> ConnectorTransporter<M> for MockTransporter<M> {
            async fn send(&mut self, data: M) -> Result<(), io::Error> {
                self.tx.send(data).await.unwrap();
                Ok(())
            }
            async fn close(&mut self) -> Result<(), io::Error> {
                Ok(())
            }
        }

        let (transporter, rx) = MockTransporter::<MediaEndpointLogRequest>::new();

        let base = ".atm0s/test/connector-queue-1";
        if let Err(e) = yaque::queue::try_clear(base) {
            log::warn!("Error trying to clear queue: {:?}", e);
        }
        let (mut queue, sender) = TransporterQueue::new(base, Box::new(transporter)).unwrap();

        let message = MediaEndpointLogRequest {
            event: Some(media_endpoint_log_request::Event::SessionEvent(SessionEvent {
                ip: "127.0.0.1".to_string(),
                version: None,
                location: None,
                token: vec![1, 2, 3, 4, 5, 6],
                ts: 0,
                session_uuid: 0,
                event: Some(session_event::Event::Stats(SessionStats {
                    receive_limit_bitrate: 0,
                    send_est_bitrate: 0,
                    sent_bytes: 0,
                    rtt: 0,
                    received_bytes: 0,
                })),
            })),
        };

        let message_c = message.clone();

        // Spawn a task to simulate receiving data
        async_std::task::spawn(async move {
            let mut sender = sender;
            sender.send(message_c.clone().encode_to_vec()).await.unwrap();
        });

        // Poll the TransporterQueue and assert the received data
        queue.poll().await.unwrap();
        let received_message = rx.recv().await.unwrap();

        assert_eq!(message, received_message);
    }

    #[async_std::test]
    async fn should_hold_msg_and_try_send_later_if_error() {
        struct MockTransporter<M> {
            tx: Sender<M>,
            fail: bool,
        }

        impl<M> MockTransporter<M> {
            pub fn new() -> (Self, Receiver<M>) {
                let (tx, rx) = channel::bounded(1);
                (Self { tx: tx.clone(), fail: true }, rx.clone())
            }
        }

        #[async_trait::async_trait]
        impl<M: Message + Clone + TryFrom<Vec<u8>>> ConnectorTransporter<M> for MockTransporter<M> {
            async fn send(&mut self, data: M) -> Result<(), io::Error> {
                if self.fail {
                    self.fail = false;
                    return Err(io::Error::new(io::ErrorKind::Other, "Error"));
                }
                self.tx.send(data).await.unwrap();
                Ok(())
            }
            async fn close(&mut self) -> Result<(), io::Error> {
                Ok(())
            }
        }

        let (transporter, rx) = MockTransporter::<MediaEndpointLogRequest>::new();

        let base = ".atm0s/test/connector-queue-2";
        if let Err(e) = yaque::queue::try_clear(base) {
            log::warn!("Error trying to clear queue: {:?}", e);
        }
        let (mut queue, sender) = TransporterQueue::new(base, Box::new(transporter)).unwrap();
        let message = MediaEndpointLogRequest {
            event: Some(media_endpoint_log_request::Event::SessionEvent(SessionEvent {
                ip: "127.0.0.1".to_string(),
                version: None,
                location: None,
                token: vec![1, 2, 3, 4, 5, 6],
                ts: 0,
                session_uuid: 0,
                event: Some(session_event::Event::Stats(SessionStats {
                    receive_limit_bitrate: 0,
                    send_est_bitrate: 0,
                    sent_bytes: 0,
                    rtt: 0,
                    received_bytes: 0,
                })),
            })),
        };

        let message_c = message.clone();

        // Spawn a task to simulate receiving data
        async_std::task::spawn(async move {
            let mut sender = sender;
            sender.send(message_c.clone().encode_to_vec()).await.unwrap();
        });

        // Poll the TransporterQueue and assert the received data
        queue.poll().await.unwrap();
        assert!(rx.try_recv().is_err());

        // Poll the TransporterQueue again and assert the received data
        queue.poll().await.unwrap();
        let received_message = rx.try_recv().unwrap();

        assert_eq!(message, received_message);
    }
}
