use std::sync::Arc;

use if_addrs::Interface;
use tokio::{sync::Mutex, time::Duration};

pub struct InterfaceMonitor {
    interval: Duration,
    interfaces: Arc<Mutex<Vec<Interface>>>,
    on_change: Box<dyn Fn(&[Interface]) + Send + Sync>,
}

impl std::fmt::Debug for InterfaceMonitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InterfaceMonitor")
            .field("interval", &self.interval)
            .field("interfaces", &self.interfaces)
            //.field("on_change", &self.on_change)
            .finish()
    }
}

impl InterfaceMonitor {
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            interfaces: Arc::new(Mutex::new(Vec::new())),
            on_change: Box::new(|_| {}),
        }
    }

    pub fn on_change<F>(&mut self, callback: F)
    where
        F: Fn(&[Interface]) + Send + Sync + 'static,
    {
        self.on_change = Box::new(callback);
    }
}
