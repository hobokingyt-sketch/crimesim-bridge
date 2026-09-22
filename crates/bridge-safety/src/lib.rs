//! Recoverable source/build promotion shared by the desktop app and process-death tests.
//! Local filesystem and trusted-code boundary; this is not a sandbox or power-loss certification.
pub mod durable;
mod files;
mod journal;
mod workspace;
pub use files::{copy_tree, fingerprint, Fingerprint};
pub use journal::{Journal, Kind, Phase, Receipt};
pub use workspace::{Recovery, Transaction, Workspace};
pub type Result<T> = std::io::Result<T>;

pub(crate) fn invalid(message: impl Into<String>) -> std::io::Error {
    std::io::Error::other(message.into())
}

pub(crate) fn checkpoint(name: &str) {
    #[cfg(feature = "fault-injection")]
    if std::env::var("BRIDGE_TEST_STOP_AT").ok().as_deref() == Some(name) {
        if let Ok(signal) = std::env::var("BRIDGE_TEST_SIGNAL") {
            std::fs::write(signal, name).expect("test rendezvous must be writable");
            loop { std::thread::sleep(std::time::Duration::from_millis(10)); }
        }
    }
    let _ = name;
}
