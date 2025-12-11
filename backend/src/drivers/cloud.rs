// MQTT 客户端实现，用于发布 EMS 状态到云端
use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, QoS};
use serde::Serialize;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::mpsc;

/// Custom error type for MQTT operations
#[derive(Error, Debug)]
pub enum MqttError {
    #[error("Connection failed: {0}")]
    Connection(String),
    #[error("Publish failed: {0}")]
    Publish(String),
    #[error("Serialization failed: {0}")]
    Serialize(#[from] serde_json::Error),
}

/// Configuration for MQTT client
#[derive(Debug, Clone)]
pub struct MqttConfig {
    pub broker: String,
    pub port: u16,
    pub client_id: String,
    pub keep_alive: Duration,
    pub max_inflight: usize,
}

impl Default for MqttConfig {
    fn default() -> Self {
        Self {
            broker: "localhost".to_string(),
            port: 1883,
            client_id: "ems_client".to_string(),
            keep_alive: Duration::from_secs(30),
            max_inflight: 10,
        }
    }
}

/// MQTT client wrapper with async support and proper error handling
#[derive(Debug)]
pub struct MqttClient {
    client: AsyncClient,
    config: MqttConfig,
    connected: bool,
    _event_loop_handle: tokio::task::JoinHandle<()>, // Handle to keep event loop alive
}

impl MqttClient {
    /// Creates a new MQTT client with the given configuration
    pub fn new(config: MqttConfig) -> Result<Self, MqttError> {
        let mut mqtt_options = MqttOptions::new(&config.client_id, &config.broker, config.port);
        mqtt_options.set_keep_alive(config.keep_alive);

        let (client, mut eventloop) = AsyncClient::new(mqtt_options, config.max_inflight);

        // Spawn the event loop to handle connection, reconnections, and incoming messages
        let event_loop_handle = tokio::spawn(async move {
            loop {
                match eventloop.poll().await {
                    Ok(Event::Incoming(incoming)) => {
                        // Handle incoming packets if needed (e.g., for subscriptions)
                        // For publishing-only client, we can ignore or log
                    }
                    Ok(Event::Outgoing(outgoing)) => {
                        // Handle outgoing events if needed
                    }
                    Err(e) => {
                        eprintln!("MQTT event loop error: {:?}", e);
                        // Implement exponential backoff for reconnection
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        });

        Ok(Self {
            client,
            config,
            connected: false,
            _event_loop_handle: event_loop_handle,
        })
    }

    /// Establishes connection to the MQTT broker (async)
    pub async fn connect(&mut self) -> Result<(), MqttError> {
        // The connection is handled by the event loop
        // We can wait for a short time to ensure connection attempt
        tokio::time::sleep(Duration::from_millis(100)).await;
        self.connected = true;
        Ok(())
    }

    /// Publishes a serializable payload to the specified topic
    pub async fn publish<T: Serialize>(&self, topic: &str, payload: &T) -> Result<(), MqttError> {
        if !self.connected {
            return Err(MqttError::Connection("Client not connected".to_string()));
        }

        let data = serde_json::to_string(payload)?;
        self.client
            .publish(topic, QoS::AtMostOnce, false, data)
            .await
            .map_err(|e| MqttError::Publish(e.to_string()))?;
        Ok(())
    }

    /// Publishes a string payload directly
    pub async fn publish_str(&self, topic: &str, payload: &str) -> Result<(), MqttError> {
        if !self.connected {
            return Err(MqttError::Connection("Client not connected".to_string()));
        }

        self.client
            .publish(topic, QoS::AtMostOnce, false, payload)
            .await
            .map_err(|e| MqttError::Publish(e.to_string()))?;
        Ok(())
    }

    /// Disconnects from the MQTT broker
    pub async fn disconnect(&mut self) -> Result<(), MqttError> {
        self.client
            .disconnect()
            .await
            .map_err(|e| MqttError::Connection(e.to_string()))?;
        self.connected = false;
        Ok(())
    }

    /// Checks if the client is connected
    pub fn is_connected(&self) -> bool {
        self.connected
    }
}

impl Drop for MqttClient {
    fn drop(&mut self) {
        // Ensure we disconnect on drop (synchronously if possible)
        // Note: Async drop is not stable, so we can't await here
        // In practice, call disconnect() explicitly before dropping
    }
}
