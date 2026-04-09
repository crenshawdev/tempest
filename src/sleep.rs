// SPDX-License-Identifier: GPL-3.0-only

use cosmic::iced::Subscription;

pub use weathervane::SleepEvent;

/// Subscribes to system suspend and resume events via weathervane.
///
/// Yields `SleepEvent::Resumed` when the system wakes from suspend.
/// Wraps the library's raw stream into an iced subscription so the
/// applet runtime can drive it.
pub fn sleep_subscription() -> Subscription<SleepEvent> {
    Subscription::run(weathervane::sleep_stream)
}
