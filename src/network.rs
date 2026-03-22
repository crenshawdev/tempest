// SPDX-License-Identifier: GPL-3.0-only

use cosmic::iced::Subscription;
use cosmic::iced_futures::Subscription as IcedSubscription;

pub use weathervane::NetworkEvent;

/// Subscribes to network connectivity changes via tempest-core.
///
/// Yields `NetworkEvent::Connected` when the system transitions to
/// full connectivity. Wraps the library's raw stream into an iced
/// subscription so the applet runtime can drive it.
pub fn network_subscription() -> Subscription<NetworkEvent> {
    IcedSubscription::run(weathervane::network_stream)
}
