//! The live broadcast server: an event fan-out over a broadcast channel.

use crate::config::BroadcastSettings;
use crate::models::Event;
use tokio::sync::broadcast;

use super::filter::BroadcastFilter;

/// A multi-consumer event stream. Each subscription receives every event
/// published after it was created; clients filter locally.
pub struct BroadcastServer {
    settings: BroadcastSettings,
    sender: broadcast::Sender<Event>,
}

/// A client-side subscription into the live stream.
pub struct Subscription {
    pub(crate) rx: broadcast::Receiver<Event>,
    pub(crate) filter: BroadcastFilter,
}

impl Subscription {
    pub fn with_filter(&mut self, filter: BroadcastFilter) -> &mut Self {
        self.filter = filter;
        self
    }

    /// Receive the next event that passes this subscription's filter.
    pub async fn recv(&mut self) -> Option<Event> {
        loop {
            match self.rx.recv().await {
                Ok(ev) => {
                    if self.filter.matches(&ev) {
                        return Some(ev);
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => return None,
            }
        }
    }
}

impl BroadcastServer {
    pub fn new(settings: BroadcastSettings) -> Self {
        let (sender, _) = broadcast::channel(1024);
        Self { settings, sender }
    }

    pub fn enabled(&self) -> bool {
        self.settings.enabled
    }

    pub fn publish(&self, ev: Event) {
        let _ = self.sender.send(ev);
    }

    pub fn subscribe(&self) -> Subscription {
        Subscription {
            rx: self.sender.subscribe(),
            filter: BroadcastFilter::default(),
        }
    }
}