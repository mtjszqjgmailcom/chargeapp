// PCS 功率转换系统
// PCS device abstraction using Modbus communication for power conversion operations

use crate::types::*;
use crate::drivers::modbus::{ModbusClient, ModbusError};
use std::io;

#[derive(Clone, Debug, Default)]
pub struct PcsDevice {
    /// Device identifier
    pub id: String,
    /// Modbus client for communication
    modbus_client: Option<ModbusClient>,
    // Cached status fields for performance
    pub power_active: f32,      // Active power in kW (positive: discharging, negative: charging)
    pub power_reactive: f32,    // Reactive power in kVAR
    pub voltage_ac: f32,        // AC voltage in V
    pub current_ac: f32,        // AC current in A
    pub voltage_dc: f32,        // DC voltage in V
    pub current_dc: f32,        // DC current in A
    pub frequency: f32,         // Grid frequency in Hz
    pub efficiency: f32,        // Efficiency percentage 0-100
    pub mode: PcsMode,          // Current operating mode
    pub fault: bool,            // Fault status
    pub fault_codes: Vec<u16>,  // Active fault codes
}

/// Operating modes for PCS device
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PcsMode {
    #[default]
    Standby,      // Idle mode
    Charging,     // Battery charging from grid
    Discharging,  // Battery discharging to grid
    GridTie,      // Grid-tied operation without battery
    OffGrid,      // Island mode
    Fault,        // Fault mode
}

impl PcsDevice {
    // Scaling constants for Modbus register values
    const SCALE_POWER: f32 = 10.0;     // Power in 0.1 kW units
    const SCALE_VOLTAGE: f32 = 10.0;   // Voltage in 0.1V units
    const SCALE_CURRENT: f32 = 10.0;   // Current in 0.1A units
    const SCALE_FREQ: f32 = 10.0;      // Frequency in 0.1 Hz units
    const SCALE_EFFICIENCY: f32 = 100.0; // Efficiency as 0-100%

    /// Create a new PCS device with Modbus communication
    ///
    /// # Arguments
    /// * `id` - Unique device identifier
    /// * `host` - Modbus server host
    /// * `port` - Modbus server port
    ///
    /// # Returns
    /// Result containing the device or IO error
    pub fn new(id: String, host: &str, port: u16) -> Result<Self, io::Error> {
        let mut modbus_client = ModbusClient::new(host, port);
        modbus_client.connect().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(Self {
            id,
            modbus_client: Some(modbus_client),
            ..Default::default()
        })
    }

    /// Read current status from the PCS device via Modbus
    ///
    /// # Returns
    /// Result containing PcsStatus or ModbusError
    pub fn read_status(&mut self) -> Result<PcsStatus, ModbusError> {
        if let Some(client) = &mut self.modbus_client {
            // Read registers: assuming mode(1), power(2)
            let registers = client.read_holding_registers(1, 2)?;

            let mode_index = registers.get(0).copied().unwrap_or(0) as usize;
            let mode = match mode_index {
                0 => "Standby",
                1 => "Charging",
                2 => "Discharging",
                3 => "GridTie",
                4 => "OffGrid",
                5 => "Fault",
                _ => "Unknown",
            }.to_string();

            let power = registers.get(1).map(|v| *v as f32 / Self::SCALE_POWER).unwrap_or(0.0);

            // Update cached fields
            self.power_active = power;
            self.power_reactive = 0.0; // TODO: Add to PcsStatus
            self.voltage_ac = 0.0; // TODO: Add to PcsStatus
            self.current_ac = 0.0; // TODO: Add to PcsStatus
            self.voltage_dc = 0.0; // TODO: Add to PcsStatus
            self.current_dc = 0.0; // TODO: Add to PcsStatus
            self.frequency = 0.0; // TODO: Add to PcsStatus
            self.efficiency = 0.0; // TODO: Add to PcsStatus
            self.mode = match mode.as_str() {
                "Standby" => PcsMode::Standby,
                "Charging" => PcsMode::Charging,
                "Discharging" => PcsMode::Discharging,
                "GridTie" => PcsMode::GridTie,
                "OffGrid" => PcsMode::OffGrid,
                "Fault" => PcsMode::Fault,
                _ => PcsMode::Standby,
            };
            self.fault = mode == "Fault"; // TODO: Add to PcsStatus
            self.fault_codes = vec![]; // TODO: Add to PcsStatus

            Ok(PcsStatus { mode, power })
        } else {
            Err(ModbusError::ConnectionFailed("Modbus client not initialized".to_string()))
        }
    }

    /// Get cached status without reading from device
    pub fn get_cached_status(&self) -> PcsStatus {
        PcsStatus {
            mode: format!("{:?}", self.mode), // TODO: Use proper enum in PcsStatus
            power: self.power_active,
        }
    }

    /// Check if device is connected
    pub fn is_connected(&self) -> bool {
        self.modbus_client.as_ref().map(|c| c.is_connected()).unwrap_or(false)
    }

    /// Set operating mode
    pub fn set_mode(&mut self, mode: PcsMode) -> Result<(), ModbusError> {
        if let Some(client) = &mut self.modbus_client {
            let mode_value = mode as u16;
            client.write_single_register(1, mode_value)?;
            Ok(())
        } else {
            Err(ModbusError::ConnectionFailed("Modbus client not initialized".to_string()))
        }
    }

    /// Write status to the PCS device (set PCS status)
    ///
    /// # Arguments
    /// * `status` - The status to write to the device
    ///
    /// # Returns
    /// Result indicating success or ModbusError
    pub fn write_status(&mut self, status: PcsStatus) -> Result<(), ModbusError> {
        if let Some(client) = &mut self.modbus_client {
            // Map mode string to register value
            let mode_value = match status.mode.as_str() {
                "Standby" => 0,
                "Charging" => 1,
                "Discharging" => 2,
                "GridTie" => 3,
                "OffGrid" => 4,
                "Fault" => 5,
                _ => 0,
            };
            let power_value = (status.power * Self::SCALE_POWER) as u16;

            // Write mode to register 1
            client.write_single_register(1, mode_value)?;
            // Write power to register 2
            client.write_single_register(2, power_value)?;
            Ok(())
        } else {
            Err(ModbusError::ConnectionFailed("Modbus client not initialized".to_string()))
        }
    }

    /// Set power setpoint for the PCS (control output power)
    ///
    /// # Arguments
    /// * `power` - Power setpoint in kW (positive for discharging, negative for charging)
    ///
    /// # Returns
    /// Result indicating success or ModbusError
    pub fn set_power_setpoint(&mut self, power: f32) -> Result<(), ModbusError> {
        if let Some(client) = &mut self.modbus_client {
            // Clamp power to reasonable range (-100 to 100 kW)
            let clamped_power = power.clamp(-100.0, 100.0);
            let register_value = ((clamped_power * Self::SCALE_POWER) as i16) as u16; // Handle negative as unsigned
            // Write to register 2 (power setpoint)
            client.write_single_register(2, register_value)?;
            Ok(())
        } else {
            Err(ModbusError::ConnectionFailed("Modbus client not initialized".to_string()))
        }
    }

}
