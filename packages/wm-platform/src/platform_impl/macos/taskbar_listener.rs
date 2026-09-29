use tokio::sync::mpsc;

use crate::Dispatcher;

/// Platform-specific implementation of [`TaskbarListener`].
///
/// macOS has no taskbar equivalent to listen to, so no events are ever
/// sent.
pub(crate) struct TaskbarListener {
  /// Held so that the channel stays open until termination, rather than
  /// closing immediately.
  event_tx: Option<mpsc::UnboundedSender<()>>,
}

impl TaskbarListener {
  /// Implements [`TaskbarListener::new`].
  #[allow(clippy::unnecessary_wraps)]
  pub(crate) fn new(
    event_tx: mpsc::UnboundedSender<()>,
    _dispatcher: &Dispatcher,
  ) -> crate::Result<Self> {
    Ok(Self {
      event_tx: Some(event_tx),
    })
  }

  /// Implements [`TaskbarListener::terminate`].
  #[allow(clippy::unnecessary_wraps)]
  pub(crate) fn terminate(&mut self) -> crate::Result<()> {
    self.event_tx.take();
    Ok(())
  }
}
