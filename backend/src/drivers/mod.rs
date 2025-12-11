// Communication drivers module
//
// This module provides drivers for various communication protocols and devices
// used in the EMS (Energy Management System) backend. Each submodule handles
// a specific communication interface or device type.

/// CAN bus communication driver
/// Handles low-level CAN socket operations for vehicle network communication
pub mod can;

/// Cloud connectivity driver
/// MQTT client implementation for publishing EMS data to cloud services
pub mod cloud;

/// GPS and 4G module driver
/// Serial communication driver for GPS positioning and 4G cellular connectivity
pub mod gps_4g;

/// Modbus TCP communication driver
/// Client implementation for Modbus protocol over TCP/IP for industrial devices
pub mod modbus;

// Optional: Re-export commonly used types for convenience
// (Uncomment if consumers frequently use these directly)
// pub use can::CanDriver;
// pub use cloud::MqttClient;
// pub use gps_4g::Gps4gModule;
// pub use modbus::ModbusClient;
