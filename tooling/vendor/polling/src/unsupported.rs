use std::fmt;
use std::io;
use std::time::Instant;

use crate::{Event, PollMode};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EventExtra {
    hup: bool,
    pri: bool,
    err: bool,
}

impl EventExtra {
    pub const fn empty() -> Self {
        Self {
            hup: false,
            pri: false,
            err: false,
        }
    }

    pub fn set_hup(&mut self, active: bool) {
        self.hup = active;
    }

    pub fn set_pri(&mut self, active: bool) {
        self.pri = active;
    }

    pub fn is_hup(&self) -> bool {
        self.hup
    }

    pub fn is_pri(&self) -> bool {
        self.pri
    }

    pub fn is_connect_failed(&self) -> Option<bool> {
        Some(self.err)
    }

    pub fn is_err(&self) -> Option<bool> {
        Some(self.err)
    }
}

#[derive(Default)]
pub struct Poller;

impl Poller {
    pub fn new() -> io::Result<Self> {
        Ok(Self)
    }

    pub fn supports_level(&self) -> bool {
        false
    }

    pub fn supports_edge(&self) -> bool {
        false
    }

    pub fn add<T>(&self, _source: T, _interest: Event, _mode: PollMode) -> io::Result<()> {
        Ok(())
    }

    pub fn modify<T>(&self, _source: T, _interest: Event, _mode: PollMode) -> io::Result<()> {
        Ok(())
    }

    pub fn delete<T>(&self, _source: T) -> io::Result<()> {
        Ok(())
    }

    pub fn wait_deadline(
        &self,
        events: &mut Events,
        _deadline: Option<Instant>,
    ) -> io::Result<()> {
        events.clear();
        Ok(())
    }

    pub fn notify(&self) -> io::Result<()> {
        Ok(())
    }
}

impl fmt::Debug for Poller {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Poller { unsupported: true }")
    }
}

pub struct Events {
    events: Vec<Event>,
}

impl Events {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            events: Vec::with_capacity(capacity),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = Event> + '_ {
        self.events.iter().copied()
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }

    pub fn capacity(&self) -> usize {
        self.events.capacity().max(1)
    }
}
