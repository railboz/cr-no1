//! Shared helpers for the integration tests.
//!
//! Only `tests/common/mod.rs` is a module (not a test target), so each test
//! binary that declares `mod common;` gets its own copy — and its own global
//! `tracing` subscriber, which is what both log-inspecting suites need.

use std::io::Write;
use std::sync::{Arc, Mutex, OnceLock};

/// In-memory stand-in for the process stdout: everything `tracing` writes lands
/// here, so a test can assert what the relay printed (and what it must not).
#[derive(Clone, Default)]
pub struct Sink(Arc<Mutex<Vec<u8>>>);

impl Sink {
    /// Everything written so far, lossy-decoded.
    pub fn contents(&self) -> String {
        let guard = self.0.lock().expect("sink lock");
        String::from_utf8_lossy(&guard).into_owned()
    }
}

impl Write for Sink {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("sink lock").extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Sink {
    type Writer = Self;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// Installs the test process's single tracing sink, at `level`, on first use.
///
/// It has to be *global*, not thread-local: `tracing` caches the interest of
/// each callsite process-wide, so a subscriber that only exists on one test
/// thread can be neutralised by another test reaching the same event first.
/// Every log-inspecting test calls `logs(level)` before its first request, and
/// the interest cache is rebuilt once the subscriber is in place.
pub fn logs(level: tracing::Level) -> &'static Sink {
    static SINK: OnceLock<Sink> = OnceLock::new();
    SINK.get_or_init(|| {
        let sink = Sink::default();
        let subscriber = tracing_subscriber::fmt()
            .with_writer(sink.clone())
            .with_max_level(level)
            .with_ansi(false)
            .finish();
        if tracing::subscriber::set_global_default(subscriber).is_ok() {
            tracing::callsite::rebuild_interest_cache();
        }
        sink
    })
}
