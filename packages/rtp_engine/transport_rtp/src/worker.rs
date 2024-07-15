use std::{collections::VecDeque, sync::Arc, time::Instant};

use media_server_secure::MediaEdgeSecure;
use sans_io_runtime::{
    backend::{BackendIncoming, BackendOutgoing},
    group_owner_type, return_if_some, TaskSwitcherChild,
};

use crate::transport::{RtpExtIn, RtpExtOut};

group_owner_type!(RtpSession);

pub enum RtpGroupIn {
    Net(BackendIncoming),
    Ext(RtpExtIn),
    Close(),
}

#[derive(Debug)]
pub enum RtpGroupOut {
    Net(BackendOutgoing),
    Ext(RtpExtOut),
    Shutdown(),
    Continue,
}

pub struct MediaRtpWorker<ES: 'static + MediaEdgeSecure> {
    queue: VecDeque<RtpGroupOut>,
    secure: Arc<ES>,
}

impl<ES: 'static + MediaEdgeSecure> MediaRtpWorker<ES> {
    pub fn new(secure: Arc<ES>) -> Self {
        Self { queue: VecDeque::new(), secure }
    }

    fn process_output(&mut self, now: Instant) {}
}

impl<ES: MediaEdgeSecure> MediaRtpWorker<ES> {
    pub fn tasks(&self) -> usize {
        0
    }

    pub fn on_tick(&mut self, now: Instant) {}

    pub fn on_event(&mut self, now: Instant, input: RtpGroupIn) {
        match input {
            RtpGroupIn::Ext(ext) => match ext {
                RtpExtIn::Ping(id) => {
                    self.queue.push_back(RtpGroupOut::Ext(RtpExtOut::Pong(id, Result::Ok("pong".to_string()))));
                }
            },
            _ => {}
        }
    }

    pub fn shutdown(&mut self, now: Instant) {}
}

impl<ES: MediaEdgeSecure> TaskSwitcherChild<RtpGroupOut> for MediaRtpWorker<ES> {
    type Time = Instant;
    fn pop_output(&mut self, now: Self::Time) -> Option<RtpGroupOut> {
        self.queue.pop_front()
    }
}
