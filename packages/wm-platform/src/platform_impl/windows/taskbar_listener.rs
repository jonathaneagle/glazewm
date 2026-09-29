use tokio::sync::mpsc;
use tracing::warn;
use windows::{
  core::w,
  Win32::{
    Foundation::HWND,
    UI::WindowsAndMessaging::{
      ChangeWindowMessageFilterEx, RegisterWindowMessageW, MSGFLT_ALLOW,
    },
  },
};

use crate::{Dispatcher, DispatcherExtWindows};

/// Platform-specific implementation of [`TaskbarListener`].
pub(crate) struct TaskbarListener {
  callback_id: Option<usize>,
  dispatcher: Dispatcher,
}

impl TaskbarListener {
  /// Implements [`TaskbarListener::new`].
  pub(crate) fn new(
    event_tx: mpsc::UnboundedSender<()>,
    dispatcher: &Dispatcher,
  ) -> crate::Result<Self> {
    // SAFETY: The message name is a valid, null-terminated wide string.
    let taskbar_created =
      unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };

    if taskbar_created == 0 {
      return Err(crate::Error::Platform(
        "Failed to register `TaskbarCreated` message.".to_string(),
      ));
    }

    // Explorer runs unelevated, so allow its broadcast through UIPI in
    // case the WM is running elevated.
    let message_window = HWND(dispatcher.message_window_handle());

    // SAFETY: The handle is the event loop's message window, which lives
    // for as long as the dispatcher.
    if let Err(err) = unsafe {
      ChangeWindowMessageFilterEx(
        message_window,
        taskbar_created,
        MSGFLT_ALLOW,
        None,
      )
    } {
      warn!("Failed to allow `TaskbarCreated` through UIPI: {}", err);
    }

    let callback_id = dispatcher.register_wndproc_callback(Box::new(
      move |_hwnd, message, _wparam, _lparam| {
        if message == taskbar_created {
          let _ = event_tx.send(());
        }

        // Leave the message unhandled, since it is a broadcast that other
        // callbacks may also need.
        None
      },
    ))?;

    Ok(Self {
      callback_id: Some(callback_id),
      dispatcher: dispatcher.clone(),
    })
  }

  /// Implements [`TaskbarListener::terminate`].
  pub(crate) fn terminate(&mut self) -> crate::Result<()> {
    if let Some(id) = self.callback_id.take() {
      self.dispatcher.deregister_wndproc_callback(id)?;
    }

    Ok(())
  }
}

impl Drop for TaskbarListener {
  fn drop(&mut self) {
    if let Err(err) = self.terminate() {
      warn!("Failed to terminate taskbar listener: {}", err);
    }
  }
}
