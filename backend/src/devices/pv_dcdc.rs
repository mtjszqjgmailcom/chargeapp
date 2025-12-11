// PV DCDC 光伏设备
// PV DCDC device abstraction using Modbus communication for DC-DC conversion operations

use crate::types::*;
use crate::drivers::modbus::{ModbusClient, ModbusError};
use std::io;

/// Operating modes for PV DCDC device
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u16)]
pub enum PvMode {
    #[default]
    Standby = 0,      // Idle mode
    MPPT = 1,         // Maximum Power Point Tracking
    ConstantVoltage = 2, // Constant voltage output
    ConstantCurrent = 3, // Constant current output
    Fault = 4,        // Fault mode
}

#[derive(Debug, Clone, Default)]
pub struct PvDevice {
    /// Device identifier
    pub id: String,
    /// Modbus client for communication
    modbus_client: Option<ModbusClient>,
    // Cached status fields for performance
    pub voltage: f32,        // DC output voltage in V
    pub current: f32,        // DC output current in A
    pub power: f32,          // DC output power in W
    pub temperature: f32,    // Operating temperature in °C
    pub efficiency: f32,     // Efficiency percentage 0-100
    pub mode: PvMode,        // Current operating mode
    pub fault: bool,         // Fault status
    pub fault_codes: Vec<u16>, // Active fault codes
    pub irradiance: f32,     // Solar irradiance in W/m² (optional sensor)
    pub mppt_voltage: f32,   // MPPT tracking voltage in V
}

impl PvDevice {
    // Scaling constants for Modbus register values
    const SCALE_VOLTAGE: f32 = 10.0;    // Voltage in 0.1V units
    const SCALE_CURRENT: f32 = 10.0;    // Current in 0.1A units
    const SCALE_POWER: f32 = 10.0;      // Power in 0.1W units
    const SCALE_TEMP: f32 = 10.0;       // Temperature in 0.1°C units (offset by 500 for negative)
    const SCALE_EFFICIENCY: f32 = 100.0; // Efficiency as 0-100%
    const SCALE_IRRADIANCE: f32 = 10.0; // Irradiance in 0.1 W/m²
    const MODE_REGISTER: u16 = 6; // Register address for operating mode

    /// Create a new PV DCDC device with Modbus communication
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

    /// Read current status from the PV DCDC device via Modbus
    ///
    /// # Returns
    /// Result containing PvStatus or ModbusError
    pub fn read_status(&mut self) -> Result<PvStatus, ModbusError> {
        if let Some(client) = &mut self.modbus_client {
            // Read registers: voltage(1), current(2), power(3), temp(4), efficiency(5), mode(6), fault(7)
            let registers = client.read_holding_registers(1, 7)?;

            let voltage = registers.get(0).map(|v| *v as f32 / Self::SCALE_VOLTAGE).unwrap_or(0.0);
            let current = registers.get(1).map(|v| *v as f32 / Self::SCALE_CURRENT).unwrap_or(0.0);
            let power = registers.get(2).map(|v| *v as f32 / Self::SCALE_POWER).unwrap_or(0.0);
            let temp_raw = registers.get(3).copied().unwrap_or(0);
            let temperature = (temp_raw as f32 - 500.0) / Self::SCALE_TEMP; // Offset for negative temps
            let efficiency = registers.get(4).map(|v| *v as f32 / Self::SCALE_EFFICIENCY).unwrap_or(0.0);

            let mode_index = registers.get(5).copied().unwrap_or(0) as usize;
            let mode = match mode_index {
                0 => PvMode::Standby,
                1 => PvMode::MPPT,
                2 => PvMode::ConstantVoltage,
                3 => PvMode::ConstantCurrent,
                4 => PvMode::Fault,
                _ => PvMode::Standby,
            };

            let fault = registers.get(6).map(|v| *v != 0).unwrap_or(false);

            // Update cached fields
            self.voltage = voltage;
            self.current = current;
            self.power = power;
            self.temperature = temperature;
            self.efficiency = efficiency;
            self.mode = mode;
            self.fault = fault;
            self.fault_codes = vec![]; // TODO: Read fault codes from additional registers
            self.irradiance = 0.0; // TODO: If sensor available
            self.mppt_voltage = 0.0; // TODO: Read MPPT voltage

            Ok(PvStatus {
                voltage,
                current,
                power,
                temperature,
                efficiency,
                fault,
            })
        } else {
            Err(ModbusError::ConnectionFailed("Modbus client not initialized".to_string()))
        }
    }

    /// Get cached status without reading from device
    pub fn get_cached_status(&self) -> PvStatus {
        PvStatus {
            voltage: self.voltage,
            current: self.current,
            power: self.power,
            temperature: self.temperature,
            efficiency: self.efficiency,
            fault: self.fault,
        }
    }

    /// Check if device is connected
    pub fn is_connected(&self) -> bool {
        self.modbus_client.as_ref().map(|c| c.is_connected()).unwrap_or(false)
    }

    /// Set operating mode
    pub fn set_mode(&mut self, mode: PvMode) -> Result<(), ModbusError> {
        if let Some(client) = &mut self.modbus_client {
            let mode_value = mode as u16;
            client.write_single_register(Self::MODE_REGISTER, mode_value)?;
            Ok(())
        } else {
            Err(ModbusError::ConnectionFailed("Modbus client not initialized".to_string()))
        }
    }

    /// Set voltage setpoint for constant voltage mode
    ///
    /// # Arguments
    /// * `voltage` - Target voltage in V
    ///
    /// # Returns
    /// Result indicating success or ModbusError
    pub fn set_voltage_setpoint(&mut self, voltage: f32) -> Result<(), ModbusError> {
        if let Some(client) = &mut self.modbus_client {
            // Clamp voltage to reasonable range (0-1000V)
            let clamped_voltage = voltage.clamp(0.0, 1000.0);
            let register_value = (clamped_voltage * Self::SCALE_VOLTAGE) as u16;
            client.write_single_register(10, register_value)?;
            Ok(())
        } else {
            Err(ModbusError::ConnectionFailed("Modbus client not initialized".to_string()))
        }
    }

    /// Set power setpoint for the PV DCDC (control output power)
    ///
    /// # Arguments
    /// * `power` - Power setpoint in W (positive for output)
    ///
    /// # Returns
    /// Result indicating success or ModbusError
    pub fn set_power_setpoint(&mut self, power: f32) -> Result<(), ModbusError> {
        if let Some(client) = &mut self.modbus_client {
            // Clamp power to reasonable range (0-10000W)
            let clamped_power = power.clamp(0.0, 10000.0);
            let register_value = (clamped_power * Self::SCALE_POWER) as u16;
            client.write_single_register(11, register_value)?;
            Ok(())
        } else {
            Err(ModbusError::ConnectionFailed("Modbus client not initialized".to_string()))
        }
    }
}
