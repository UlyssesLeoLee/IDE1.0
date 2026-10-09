// daemon.rs — 常駐機能 (sakura 14.1)
//
// バックグラウンドで常駐するサービス.
// 実装: tokio 不要, std::thread で動作.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// 常駐サービス
pub struct Daemon {
    name: String,
    running: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl Daemon {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            running: Arc::new(AtomicBool::new(false)),
            handle: None,
        }
    }

    /// サービス起動
    /// `tick` は 1 サイクル毎の処理
    pub fn start<F>(&mut self, interval: Duration, mut tick: F)
    where
        F: FnMut() + Send + 'static,
    {
        if self.running.load(Ordering::SeqCst) {
            return;
        }
        self.running.store(true, Ordering::SeqCst);
        let running = self.running.clone();
        let handle = thread::spawn(move || {
            while running.load(Ordering::SeqCst) {
                tick();
                thread::sleep(interval);
            }
        });
        self.handle = Some(handle);
    }

    /// 停止
    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn test_start_and_stop() {
        let mut d = Daemon::new("test");
        d.start(Duration::from_millis(20), || {});
        assert!(d.is_running());
        d.stop();
        assert!(!d.is_running());
    }

    #[test]
    fn test_tick_called() {
        let mut d = Daemon::new("counter");
        let count = Arc::new(AtomicUsize::new(0));
        let c2 = count.clone();
        d.start(Duration::from_millis(10), move || {
            c2.fetch_add(1, Ordering::SeqCst);
        });
        thread::sleep(Duration::from_millis(100));
        d.stop();
        let final_count = count.load(Ordering::SeqCst);
        assert!(final_count > 0, "expected tick to be called, got {}", final_count);
    }

    #[test]
    fn test_double_start_noop() {
        let mut d = Daemon::new("a");
        d.start(Duration::from_millis(10), || {});
        d.start(Duration::from_millis(10), || {}); // noop
        assert!(d.is_running());
        d.stop();
    }

    #[test]
    fn test_name() {
        let d = Daemon::new("my-daemon");
        assert_eq!(d.name(), "my-daemon");
    }

    #[test]
    fn test_drop_stops() {
        let count = Arc::new(AtomicUsize::new(0));
        let c2 = count.clone();
        {
            let mut d = Daemon::new("auto-stop");
            d.start(Duration::from_millis(10), move || {
                c2.fetch_add(1, Ordering::SeqCst);
            });
            thread::sleep(Duration::from_millis(50));
        } // Drop here
        thread::sleep(Duration::from_millis(50));
        // 停止しているので count は増え続ける
        let c1 = count.load(Ordering::SeqCst);
        thread::sleep(Duration::from_millis(50));
        let c2_v = count.load(Ordering::SeqCst);
        assert_eq!(c1, c2_v, "daemon should be stopped after drop");
    }
}
