// SPDX-License-Identifier: GPL-3.0-only

use cosmic::iced::Subscription;
use cosmic::iced_futures::Subscription as IcedSubscription;

/// Network connectivity events from NetworkManager.
#[derive(Debug, Clone)]
pub enum NetworkEvent {
    /// Network connectivity was established or restored.
    Connected,
}

/// NM_STATE_CONNECTED_GLOBAL from the NetworkManager D-Bus API.
/// Indicates full network connectivity is available.
const NM_STATE_CONNECTED_GLOBAL: u32 = 70;

/// Subscribes to NetworkManager's StateChanged signal over D-Bus.
///
/// Yields `NetworkEvent::Connected` when the system transitions to full
/// connectivity. Falls back to an empty stream if NetworkManager is
/// unavailable (no D-Bus, container environment, etc).
pub fn network_subscription() -> Subscription<NetworkEvent> {
    IcedSubscription::run(|| {
        async_stream::stream! {
            let Ok(connection) = zbus::Connection::system().await else {
                tracing::warn!("Could not connect to system D-Bus, network monitoring disabled");
                // Park the stream forever so the subscription stays alive but idle
                std::future::pending::<()>().await;
                return;
            };

            let rule = "type='signal',\
                        sender='org.freedesktop.NetworkManager',\
                        interface='org.freedesktop.NetworkManager',\
                        member='StateChanged',\
                        path='/org/freedesktop/NetworkManager'";

            if let Err(e) = connection
                .call_method(
                    Some("org.freedesktop.DBus"),
                    "/org/freedesktop/DBus",
                    Some("org.freedesktop.DBus"),
                    "AddMatch",
                    &rule,
                )
                .await
            {
                tracing::warn!("Failed to subscribe to NetworkManager signals: {}", e);
                std::future::pending::<()>().await;
                return;
            }

            tracing::info!("Listening for NetworkManager connectivity changes");

            let mut stream = zbus::MessageStream::from(&connection);

            use futures::StreamExt;
            while let Some(Ok(msg)) = stream.next().await {
                // Only process signals matching our StateChanged subscription
                let header = msg.header();
                let matches_member = header.member().is_some_and(|m| m == "StateChanged");
                let matches_interface = header
                    .interface()
                    .is_some_and(|i| i == "org.freedesktop.NetworkManager");

                if !matches_member || !matches_interface {
                    continue;
                }

                if let Ok(body) = msg.body().deserialize::<(u32,)>() {
                    let state = body.0;
                    tracing::debug!("NetworkManager state changed: {}", state);

                    if state == NM_STATE_CONNECTED_GLOBAL {
                        tracing::info!("Network connectivity restored");
                        yield NetworkEvent::Connected;
                    }
                }
            }
        }
    })
}
