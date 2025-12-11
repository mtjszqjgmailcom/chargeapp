// Genset 发电机设备
// Genset device abstraction using Modbus communication

use crate::types::*;
use crate::drivers::modbus::{ModbusClient, ModbusError};
use std::io;

#[derive(Debug, Clone, Default)]
pub struct GensetDevice {
    pub id: String,
    modbus_client: Option<ModbusClient>,
    // Cached status for quick access, updated on read_status
    pub running: bool,
    pub power_output: f32,
    pub fuel_level: f32,
    pub voltage: f32,
    pub current: f32,
    pub frequency: f32,
    pub engine_hours: u32,
    pub temperature: f32,
}

impl GensetDevice {
    // Scaling constants for Modbus register values
    const SCALE_POWER: f32 = 10.0; // Power in 0.1 kW units
    const SCALE_FUEL: f32 = 100.0; // Fuel level as 0-100%
    const SCALE_VOLTAGE: f32 = 10.0; // Voltage in 0.1V units
    const SCALE_CURRENT: f32 = 10.0; // Current in 0.1A units
    const SCALE_FREQ: f32 = 10.0; // Frequency in 0.1 Hz units
    const SCALE_TEMP: f32 = 10.0; // Temperature in 0.1°C units

    /// Create a new GensetDevice with Modbus connection
    ///
    /// # Arguments
    /// * `id` - Device identifier
    /// * `host` - Modbus server host
    /// * `port` - Modbus server port
    ///
    /// # Returns
    /// Result containing the device or an IO error
    pub fn new(id: String, host: &str, port: u16) -> Result<Self, io::Error> {
        let mut modbus_client = ModbusClient::new(host, port);
        modbus_client.connect().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        Ok(Self {
            id,
            modbus_client: Some(modbus_client),
            running: false,
            power_output: 0.0,
            fuel_level: 0.0,
            voltage: 0.0,
            current: 0.0,
            frequency: 0.0,
            engine_hours: 0,
            temperature: 0.0,
        })
    }

    /// Read the current status from the genset via Modbus
    ///
    /// # Returns
    /// Result containing GensetStatus or ModbusError
    pub fn read_status(&mut self) -> Result<GensetStatus, ModbusError> {
        if let Some(client) = &mut self.modbus_client {
            // Read running status from coil (address 0)
            let running_coil = client.read_coils(0, 1)?;
            let running = running_coil.get(0).copied().unwrap_or(false);

            // Read registers for other parameters
            // Assuming registers: power(1), fuel(2), voltage(3), current(4), freq(5), hours(6-7), temp(8)
            let registers = client.read_holding_registers(1, 8)?;

            let power_output = registers.get(0).map(|v| *v as f32 / Self::SCALE_POWER).unwrap_or(0.0);
            let fuel_level = registers.get(1).map(|v| *v as f32 / Self::SCALE_FUEL).unwrap_or(0.0);
            let voltage = registers.get(2).map(|v| *v as f32 / Self::SCALE_VOLTAGE).unwrap_or(0.0);
            let current = registers.get(3).map(|v| *v as f32 / Self::SCALE_CURRENT).unwrap_or(0.0);
            let frequency = registers.get(4).map(|v| *v as f32 / Self::SCALE_FREQ).unwrap_or(0.0);
            let engine_hours = if registers.len() >= 7 {
                ((registers[5] as u32) << 16) | (registers[6] as u32)
            } else { 0 };
            let temperature = registers.get(7).map(|v| *v as f32 / Self::SCALE_TEMP).unwrap_or(0.0);

            // Update cached status
            self.running = running;
            self.power_output = power_output;
            self.fuel_level = fuel_level;
            self.voltage = voltage;
            self.current = current;
            self.frequency = frequency;
            self.engine_hours = engine_hours;
            self.temperature = temperature;

            Ok(GensetStatus {
                running,
                power_output,
                fuel_level,
                voltage,
                current,
                frequency,
                engine_hours,
                temperature,
            })
        } else {
            Err(ModbusError::ConnectionFailed("Modbus client not initialized".to_string()))
        }
    }

    /// Get cached status without reading from device
    pub fn get_cached_status(&self) -> GensetStatus {
        GensetStatus {
            running: self.running,
            power_output: self.power_output,
            fuel_level: self.fuel_level,
            voltage: self.voltage,
            current: self.current,
            frequency: self.frequency,
            engine_hours: self.engine_hours,
            temperature: self.temperature,
        }
    }

    /// Check if device is connected
    pub fn is_connected(&self) -> bool {
        self.modbus_client.as_ref().map(|c| c.is_connected()).unwrap_or(false)
    }

    /// Start the genset engine
    ///
    /// # Returns
    /// Result indicating success or ModbusError
    pub fn start_engine(&mut self) -> Result<(), ModbusError> {
        if let Some(client) = &mut self.modbus_client {
            // Write to coil 1 to start engine
            client.write_single_coil(1, true)?;
            // Update cached status
            self.running = true;
            Ok(())
        } else {
            Err(ModbusError::ConnectionFailed("Modbus client not initialized".to_string()))
        }
    }

    /// Stop the genset engine
    ///
    /// # Returns
    /// Result indicating success or ModbusError
    pub fn stop_engine(&mut self) -> Result<(), ModbusError> {
        if let Some(client) = &mut self.modbus_client {
            // Write to coil 1 to stop engine
            client.write_single_coil(1, false)?;
            // Update cached status
            self.running = false;
            Ok(())
        } else {
            Err(ModbusError::ConnectionFailed("Modbus client not initialized".to_string()))
        }
    }

    /// Set power setpoint for the genset
    ///
    /// # Arguments
    /// * `power` - Power setpoint in kW
    ///
    /// # Returns
    /// Result indicating success or ModbusError
    pub fn set_power_setpoint(&mut self, power: f32) -> Result<(), ModbusError> {
        if let Some(client) = &mut self.modbus_client {
            // Clamp power to reasonable range (0-1000 kW)
            let clamped_power = power.clamp(0.0, 1000.0);
            let register_value = (clamped_power * Self::SCALE_POWER) as u16;
            // Write to register 9 (power setpoint)
            client.write_single_register(9, register_value)?;
            Ok(())
        } else {
            Err(ModbusError::ConnectionFailed("Modbus client not initialized".to_string()))
        }
    }
}
