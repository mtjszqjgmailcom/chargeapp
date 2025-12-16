// Charger 充电器设备
// Charger device abstraction using CAN communication for charging control

use crate::types::*;
use crate::drivers::can::CanDriver;
use socketcan::{CanFrame, CanDataFrame, EmbeddedFrame, StandardId, Id};
use serde::{Serialize, Deserialize};
use std::io;

/// Charger device with CAN communication
#[derive(Debug, Default)]
pub struct ChargerDevice {
    /// Device identifier
    pub id: String,
    /// CAN driver for communication
    can_driver: Option<CanDriver>,
    // Cached status fields for performance
    pub charging: bool,         // Charging state
    pub power: f32,             // Charging power in kW
    pub voltage: f32,           // Output voltage in V
    pub current: f32,           // Output current in A
    pub temperature: f32,       // Operating temperature in °C
    pub efficiency: f32,        // Efficiency percentage 0-100
    pub fault: bool,            // Fault status
    pub fault_codes: Vec<u16>,  // Active fault codes
}

/// 车载电池信息
/// Vehicle battery pack status information for car-charging station interaction and cloud transmission
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CarBattery {
    /// Battery pack ID
    pub id: String,
    /// State of charge (0-100%)
    pub soc: f32,
    /// Battery pack voltage in V
    pub voltage: f32,
    /// Battery pack current in A (positive: discharging, negative: charging)
    pub current: f32,
    /// Maximum cell voltage in V
    pub max_cell_voltage: f32,
    /// Minimum cell voltage in V
    pub min_cell_voltage: f32,
    /// Cell temperature in °C
    pub cell_temperature: f32,
    /// Protection board temperature in °C
    pub board_temperature: f32,
    /// Maximum charge power in kW
    pub max_charge_power: f32,
    /// Maximum discharge power in kW
    pub max_discharge_power: f32,
    /// Battery health status (0-100%)
    pub health: f32,
    /// Fault status
    pub fault: bool,
    /// Active fault codes
    pub fault_codes: Vec<u16>,
}

/// Operating modes for charger
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChargerMode {
    #[default]
    Standby,      // Idle, not charging
    Charging,     // Active charging
    Fault,        // Fault condition
}

impl ChargerDevice {
    // === CAN Data Scaling Constants ===
    // These constants define the scaling factors for encoding/decoding CAN message fields
    // to optimize data transmission within the 8-byte CAN frame limit
    const SCALE_POWER: f32 = 10.0;         // Power scaled to 0.1 kW units (resolution: 0.1 kW, range: 0-6553.5 kW)
    const SCALE_VOLTAGE: f32 = 10.0;       // Voltage scaled to 0.1V units (resolution: 0.1V, range: 0-6553.5V)
    const SCALE_CURRENT: f32 = 10.0;       // Current scaled to 0.1A units (resolution: 0.1A, range: 0-6553.5A)
    const SCALE_TEMP: f32 = 100.0;         // Temperature scaled to 0.01°C units (resolution: 0.01°C, range: -327.68°C to 327.67°C)
    const SCALE_EFFICIENCY: f32 = 100.0;   // Efficiency as percentage (0-100%, no scaling needed)

    // === CAN Frame Constraints ===
    const MAX_CAN_DATA_SIZE: usize = 8;    // Maximum bytes in a standard CAN frame
    const MAX_FAULT_CODES: usize = 2;      // Maximum fault codes that fit within 8-byte limit (base 12 bytes + 4 bytes for 2 codes = 16, but limited to 8)

    // === CAN ID Constants ===
    const STATUS_REQUEST_ID: StandardId = StandardId::new(0x200).unwrap();
    const STATUS_RESPONSE_ID: StandardId = StandardId::new(0x201).unwrap();

    /// Helper method to update cached fields from status
    fn update_cache(&mut self, status: &ChargerStatus) {
        self.charging = status.charging;
        self.power = status.power;
        self.voltage = status.voltage;
        self.current = status.current;
        self.temperature = status.temperature;
        self.efficiency = status.efficiency;
        self.fault = status.fault;
        self.fault_codes = status.fault_codes.clone(); // Cloning is acceptable for small Vec<u16>
    }

    /// Create a new charger device with CAN communication
    ///
    /// # Arguments
    /// * `id` - Unique device identifier
    /// * `can_interface` - CAN interface name (e.g., "can0")
    ///
    /// # Returns
    /// Result containing the device or IO error
    pub fn new(id: String, can_interface: &str) -> Result<Self, io::Error> {
        let mut can_driver = CanDriver::new(can_interface);
        can_driver.connect().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(Self {
            id,
            can_driver: Some(can_driver),
            ..Default::default()
        })
    }

    /// Pack ChargerStatus into CAN frame data
    /// Format: charging(u8), power(u16), voltage(u16), current(u16), temp(i16), efficiency(u8), fault(u8), fault_count(u8), fault_codes(u16[])
    /// Total: 12 bytes + 2*fault_count bytes
    /// Limited to MAX_CAN_DATA_SIZE (8 bytes), so fault_count is clamped to fit
    fn pack_charger_status(status: &ChargerStatus) -> Result<Vec<u8>, io::Error> {
        let mut data = Vec::with_capacity(16);

        // Clamp values to expected ranges
        let charging = if status.charging { 1u8 } else { 0u8 };
        let power = (status.power.clamp(0.0, 100.0) * Self::SCALE_POWER) as u16;
        let voltage = (status.voltage.clamp(0.0, 1000.0) * Self::SCALE_VOLTAGE) as u16;
        let current = (status.current.clamp(0.0, 200.0) * Self::SCALE_CURRENT) as u16;
        let temp = (status.temperature.clamp(-50.0, 100.0) * Self::SCALE_TEMP) as i16;
        let efficiency = status.efficiency.clamp(0.0, 100.0) as u8;
        let fault = if status.fault { 1u8 } else { 0u8 };
        let fault_count = status.fault_codes.len().min(Self::MAX_FAULT_CODES) as u8;

        data.push(charging);
        data.extend_from_slice(&power.to_be_bytes());
        data.extend_from_slice(&voltage.to_be_bytes());
        data.extend_from_slice(&current.to_be_bytes());
        data.extend_from_slice(&temp.to_be_bytes());
        data.push(efficiency);
        data.push(fault);
        data.push(fault_count);
        for code in &status.fault_codes[..fault_count as usize] {
            data.extend_from_slice(&code.to_be_bytes());
        }

        if data.len() > Self::MAX_CAN_DATA_SIZE {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Packed data exceeds CAN frame size limit"));
        }

        Ok(data)
    }

    /// Unpack CAN frame data into ChargerStatus
    /// Expects exactly 12 bytes + 2*fault_count bytes: charging(u8), power(u16), voltage(u16), current(u16), temp(i16), efficiency(u8), fault(u8), fault_count(u8)
    /// Followed by fault codes if fault_count > 0
    /// Returns None if data is invalid or too short
    fn unpack_charger_status(data: &[u8]) -> Option<ChargerStatus> {
        if data.len() < 12 {
            return None;
        }

        let charging = data[0] != 0;
        let power = (u16::from_be_bytes([data[1], data[2]]) as f32 / Self::SCALE_POWER).max(0.0);
        let voltage = (u16::from_be_bytes([data[3], data[4]]) as f32 / Self::SCALE_VOLTAGE).max(0.0);
        let current = (u16::from_be_bytes([data[5], data[6]]) as f32 / Self::SCALE_CURRENT).max(0.0);
        let temp = (i16::from_be_bytes([data[7], data[8]]) as f32 / Self::SCALE_TEMP).clamp(-50.0, 100.0);
        let efficiency = (data[9] as f32).clamp(0.0, 100.0);
        let fault = data[10] != 0;
        let fault_count = (data[11] as usize).min(Self::MAX_FAULT_CODES);

        let mut fault_codes = Vec::new();
        let mut offset = 12;
        for _ in 0..fault_count {
            if offset + 2 <= data.len() {
                let code = u16::from_be_bytes([data[offset], data[offset + 1]]);
                fault_codes.push(code);
                offset += 2;
            }
        }

        Some(ChargerStatus {
            charging,
            power,
            voltage,
            current,
            temperature: temp,
            efficiency,
            fault,
            fault_codes,
        })
    }

    /// Pack CarBattery into CAN frame data (simplified format, may span multiple frames)
    /// Note: In full implementation, this would handle multi-frame CAN messages
    fn pack_car_battery(battery: &CarBattery) -> Result<Vec<u8>, io::Error> {
        // Simplified packing: pack key fields into a single frame where possible
        // id is not packed (assume known), pack numeric fields
        let mut data = Vec::with_capacity(16);

        let soc = (battery.soc.clamp(0.0, 100.0) * Self::SCALE_EFFICIENCY) as u8;
        let voltage = (battery.voltage.clamp(0.0, 1000.0) * Self::SCALE_VOLTAGE) as u16;
        let current = ((battery.current + 1000.0).clamp(0.0, 2000.0) * Self::SCALE_CURRENT) as u16; // offset for negative
        let max_cell_v = (battery.max_cell_voltage.clamp(0.0, 10.0) * 100.0) as u16; // 0.01V
        let min_cell_v = (battery.min_cell_voltage.clamp(0.0, 10.0) * 100.0) as u16;
        let cell_temp = ((battery.cell_temperature + 50.0).clamp(0.0, 150.0) * Self::SCALE_TEMP / 100.0) as u8; // offset
        let board_temp = ((battery.board_temperature + 50.0).clamp(0.0, 150.0) * Self::SCALE_TEMP / 100.0) as u8;
        let max_charge = (battery.max_charge_power.clamp(0.0, 100.0) * Self::SCALE_POWER) as u16;
        let health = (battery.health.clamp(0.0, 100.0) * Self::SCALE_EFFICIENCY) as u8;

        data.push(soc);
        data.extend_from_slice(&voltage.to_be_bytes());
        data.extend_from_slice(&current.to_be_bytes());
        data.extend_from_slice(&max_cell_v.to_be_bytes());
        data.extend_from_slice(&min_cell_v.to_be_bytes());
        data.push(cell_temp);
        data.push(board_temp);
        data.extend_from_slice(&max_charge.to_be_bytes());
        data.push(health);

        if data.len() > Self::MAX_CAN_DATA_SIZE * 2 { // allow 2 frames
            return Err(io::Error::new(io::ErrorKind::InvalidData, "CarBattery data too large for CAN"));
        }

        Ok(data)
    }

    /// Unpack CAN frame data into CarBattery (simplified, assumes single frame for key fields)
    fn unpack_car_battery(data: &[u8]) -> Option<CarBattery> {
        if data.len() < 14 { // minimum for packed fields
            return None;
        }

        let soc = data[0] as f32 / Self::SCALE_EFFICIENCY;
        let voltage = u16::from_be_bytes([data[1], data[2]]) as f32 / Self::SCALE_VOLTAGE;
        let current = u16::from_be_bytes([data[3], data[4]]) as f32 / Self::SCALE_CURRENT - 1000.0; // offset
        let max_cell_voltage = u16::from_be_bytes([data[5], data[6]]) as f32 / 100.0;
        let min_cell_voltage = u16::from_be_bytes([data[7], data[8]]) as f32 / 100.0;
        let cell_temperature = data[9] as f32 * 100.0 / Self::SCALE_TEMP - 50.0;
        let board_temperature = data[10] as f32 * 100.0 / Self::SCALE_TEMP - 50.0;
        let max_charge_power = u16::from_be_bytes([data[11], data[12]]) as f32 / Self::SCALE_POWER;
        let health = data[13] as f32 / Self::SCALE_EFFICIENCY;

        Some(CarBattery {
            id: String::new(), // assume set separately or from context
            soc: soc.clamp(0.0, 100.0),
            voltage: voltage.max(0.0),
            current,
            max_cell_voltage: max_cell_voltage.max(0.0),
            min_cell_voltage: min_cell_voltage.max(0.0),
            cell_temperature: cell_temperature.clamp(-50.0, 100.0),
            board_temperature: board_temperature.clamp(-50.0, 100.0),
            max_charge_power: max_charge_power.max(0.0),
            max_discharge_power: 0.0, // not packed, set default
            health: health.clamp(0.0, 100.0),
            fault: false, // not packed
            fault_codes: Vec::new(),
        })
    }

    /// Read current status from the charger device via CAN
    ///
    /// # Returns
    /// Result containing ChargerStatus or IO error
    pub fn read_status(&mut self) -> Result<ChargerStatus, io::Error> {
        // Early return if driver not initialized
        let driver = self.can_driver.as_ref().ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotConnected, "CAN driver not initialized")
        })?;

        // Prepare and send request frame
        let request_data = &[0x01];
        let request_frame = CanDataFrame::new(Self::STATUS_REQUEST_ID, request_data)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid request frame data"))?;
        driver.send_frame(&CanFrame::Data(request_frame))
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        // Receive response (TODO: Add timeout/retry logic in production, e.g., via async with tokio::time::timeout)
        // For sync code, consider a loop with std::thread::sleep and a timeout counter
        let response_frame = driver.recv_frame()
            .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

        // Validate response and extract data frame
        let data_frame = match response_frame {
            CanFrame::Data(df) if df.id() == Id::Standard(Self::STATUS_RESPONSE_ID) => df,
            CanFrame::Data(_) => return Err(io::Error::new(io::ErrorKind::InvalidData, "Unexpected CAN frame ID")),
            _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "Expected data frame")),
        };

        // Unpack status, handling invalid data
        let status = Self::unpack_charger_status(&data_frame.data()[..data_frame.dlc() as usize])
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Invalid CAN frame data"))?;

        // Update cache and return
        self.update_cache(&status);
        Ok(status)
    }

    /// Read car battery information from the vehicle via CAN
    ///
    /// # Returns
    /// Result containing CarBattery or IO error
    pub fn read_car_battery(&mut self) -> Result<CarBattery, io::Error> {
        if let Some(driver) = &self.can_driver {
            // Send read request for battery data (assume CAN ID 0x205)
            let request_frame = CanFrame::Data(CanDataFrame::new(StandardId::new(0x205).unwrap(), &[0x01]).unwrap());
            driver.send_frame(&request_frame).map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;

            // Receive response
            let response_frame = driver.recv_frame().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            match response_frame {
                CanFrame::Data(data_frame) if data_frame.id() == Id::Standard(StandardId::new(0x206).unwrap()) => {
                    let battery = Self::unpack_car_battery(&data_frame.data()[..data_frame.dlc() as usize]).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Invalid CAN frame data for CarBattery"))?;
                    // Optionally cache if needed, but for now return directly
                    Ok(battery)
                }
                CanFrame::Data(_) => Err(io::Error::new(io::ErrorKind::InvalidData, "Unexpected CAN frame ID for CarBattery")),
                _ => Err(io::Error::new(io::ErrorKind::InvalidData, "Expected data frame for CarBattery")),
            }
        } else {
            Err(io::Error::new(io::ErrorKind::NotConnected, "CAN driver not initialized"))
        }
    }

    /// Get cached status without reading from device
    pub fn get_cached_status(&self) -> ChargerStatus {
        ChargerStatus {
            charging: self.charging,
            power: self.power,
            voltage: self.voltage,
            current: self.current,
            temperature: self.temperature,
            efficiency: self.efficiency,
            fault: self.fault,
            fault_codes: self.fault_codes.clone(),
        }
    }

    /// Check if device is connected
    pub fn is_connected(&self) -> bool {
        self.can_driver.as_ref().map(|d| d.is_connected()).unwrap_or(false)
    }

    /// Set charging mode
    pub fn set_mode(&self, mode: ChargerMode) -> Result<(), io::Error> {
        if let Some(driver) = &self.can_driver {
            let mode_value = match mode {
                ChargerMode::Standby => 0u8,
                ChargerMode::Charging => 1u8,
                ChargerMode::Fault => 2u8,
            };
            let frame = CanFrame::Data(CanDataFrame::new(StandardId::new(0x202).unwrap(), &[mode_value]).unwrap());
            driver.send_frame(&frame).map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
        } else {
            Err(io::Error::new(io::ErrorKind::NotConnected, "CAN driver not initialized"))
        }
    }

    /// Write status to the charger device
    ///
    /// # Arguments
    /// * `status` - The status to write to the device
    ///
    /// # Returns
    /// Result indicating success or IO error
    pub fn write_status(&self, status: ChargerStatus) -> Result<(), io::Error> {
        if let Some(driver) = &self.can_driver {
            let data = Self::pack_charger_status(&status)?;
            let frame = CanFrame::Data(CanDataFrame::new(StandardId::new(0x203).unwrap(), &data).unwrap());
            driver.send_frame(&frame).map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
        } else {
            Err(io::Error::new(io::ErrorKind::NotConnected, "CAN driver not initialized"))
        }
    }

    /// Set power setpoint for the charger
    ///
    /// # Arguments
    /// * `power` - Power setpoint in kW (0 to disable charging)
    ///
    /// # Returns
    /// Result indicating success or IO error
    pub fn set_power_setpoint(&self, power: f32) -> Result<(), io::Error> {
        if let Some(driver) = &self.can_driver {
            // Clamp power to reasonable range
            let clamped_power = power.clamp(0.0, 50.0);
            let power_value = (clamped_power * Self::SCALE_POWER) as u16;
            let data = power_value.to_be_bytes();
            let frame = CanFrame::Data(CanDataFrame::new(StandardId::new(0x204).unwrap(), &data).unwrap());
            driver.send_frame(&frame).map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))
        } else {
            Err(io::Error::new(io::ErrorKind::NotConnected, "CAN driver not initialized"))
        }
    }
}
