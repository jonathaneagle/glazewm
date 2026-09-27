mod ignore_window;
mod manage_window;
mod move_window_in_direction;
mod move_window_to_workspace;
// Only Windows cloaks windows, so only Windows can orphan them.
#[cfg(target_os = "windows")]
mod reattach_windows;
mod resize_window;
mod run_window_rules;
mod set_window_position;
mod set_window_size;
mod unmanage_window;
mod update_window_state;

pub use ignore_window::*;
pub use manage_window::*;
pub use move_window_in_direction::*;
pub use move_window_to_workspace::*;
#[cfg(target_os = "windows")]
pub use reattach_windows::*;
pub use resize_window::*;
pub use run_window_rules::*;
pub use set_window_position::*;
pub use set_window_size::*;
pub use unmanage_window::*;
pub use update_window_state::*;
