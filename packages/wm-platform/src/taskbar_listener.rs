use tokio::sync::mpsc;

use crate::{platform_impl, Dispatcher};

/// A listener for the native taskbar being created.
///
/// A newly created taskbar has no record of entries removed from the
/// previous one, so listeners should resync taskbar entries on each event.
///
/// # Platform-specific
///
/// - **Windows**: Fires when Explorer creates its taskbar, which happens
///   at login and whenever Explorer restarts. Corresponds to the
///   `TaskbarCreated` broadcast message.
/// - **macOS**: Never fires, since the Dock has no equivalent.
pub struct TaskbarListener {
  event_rx: mpsc::UnboundedReceiver<()>,

  /// Inner platform-specific taskbar listener.
  inner: platform_impl::TaskbarListener,
}

impl TaskbarListener {
  /// Creates a new [`TaskbarListener`].
  pub fn new(dispatcher: &Dispatcher) -> crate::Result<Self> {
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    let inner = platform_impl::TaskbarListener::new(event_tx, dispatcher)?;
    Ok(Self { event_rx, inner })
  }

  /// Returns when the taskbar is next created.
  ///
  /// Returns `None` if the channel has been closed.
  pub async fn next_event(&mut self) -> Option<()> {
    self.event_rx.recv().await
  }

  /// Terminates the taskbar listener.
  pub fn terminate(&mut self) -> crate::Result<()> {
    self.inner.terminate()
  }
}
