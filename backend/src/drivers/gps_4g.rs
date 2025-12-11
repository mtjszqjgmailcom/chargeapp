// 4G + GPS 模块 (AT 指令串口)
// Handles 4G communication and GPS positioning via AT commands over serial

use std::io::{BufRead, BufReader, Write};
use std::time::Duration;
use crate::types::GpsData;

pub struct Gps4gDriver {
    port: Option<Box<dyn serialport::SerialPort>>,
    is_connected: bool,
}

impl Gps4gDriver {
    /// Creates a new Gps4gDriver with the specified serial port path.
    /// Opens the serial port at 115200 baud with a 1-second timeout.
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // For now, don't open port in new, do in init
        Ok(Self {
            port: None,
            is_connected: false,
        })
    }

    /// Initializes the 4G and GPS modules by opening serial port and sending basic AT commands.
    pub fn init(&mut self, port_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let port = serialport::new(port_path, 115200)
            .timeout(Duration::from_millis(1000))
            .open()?;
        self.port = Some(port);
        self.send_at_command("AT")?;
        self.send_at_command("AT+CGPSPWR=1")?; // Enable GPS power (example command, adjust based on module)
        self.is_connected = true;
        Ok(())
    }

    /// Sends an AT command to the module and returns the response.
    pub fn send_at_command(&mut self, command: &str) -> Result<String, Box<dyn std::error::Error>> {
        if let Some(ref mut port) = self.port {
            let cmd = format!("{}\r\n", command);
            port.write_all(cmd.as_bytes())?;
            port.flush()?;
            let mut reader = BufReader::new(port.try_clone()?);
            let mut response = String::new();
            reader.read_line(&mut response)?;
            Ok(response.trim().to_string())
        } else {
            Err("Serial port not available".into())
        }
    }

    /// Reads GPS data from the module.
    pub fn read_gps_data(&mut self) -> Result<GpsData, Box<dyn std::error::Error>> {
        let response = self.send_at_command("AT+CGPSINF=0")?; // Request GPS info (example command)
        // Parse the response (placeholder parsing, actual format depends on module)
        // Assumed format: +CGPSINF: <utc>,<lat>,<lon>,<alt>,<speed>,...
        let parts: Vec<&str> = response.split(',').collect();
        if parts.len() >= 5 {
            let latitude: f64 = parts[1].parse().unwrap_or(0.0);
            let longitude: f64 = parts[2].parse().unwrap_or(0.0);
            let altitude: f32 = parts[3].parse().unwrap_or(0.0);
            let speed: f32 = parts[4].parse().unwrap_or(0.0);
            Ok(GpsData {
                latitude,
                longitude,
                altitude,
                speed,
                timestamp: parts[0].to_string(),
            })
        } else {
            Err("Invalid GPS data response".into())
        }
    }

    /// Establishes a 4G network connection.
    pub fn connect_4g(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.send_at_command("AT+CREG?")?; // Check registration status (placeholder)
        // Additional commands to connect, e.g., AT+CGATT=1
        self.send_at_command("AT+CGATT=1")?;
        self.is_connected = true;
        Ok(())
    }

    /// Checks if the driver is connected.
    pub fn is_connected(&self) -> bool {
        self.is_connected && self.port.is_some()
    }
}
