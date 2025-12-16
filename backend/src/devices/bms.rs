// BMS 电池管理系统
// Battery device abstraction using CAN communication for separation of concerns

use crate::types::*;
use crate::drivers::can::CanDriver;
use socketcan::{CanFrame, CanDataFrame, EmbeddedFrame, StandardId, Id};
use std::io;

#[derive(Debug, Default)]
pub struct BatteryDevice {
    pub id: String,
    can_driver: Option<CanDriver>,
    // Basic battery pack status
    pub voltage: f32,
    pub current: f32,
    pub soc: f32,
    pub temperature: f32,
    pub capacity: f32,
    pub sop_charge: f32,
    pub sop_discharge: f32,
}

impl BatteryDevice {
    // Scaling constants for packing/unpacking
    const SCALE_PERCENT: f32 = 100.0; // For SOC, SOP as 0-100%
    const SCALE_VOLTAGE: f32 = 100.0; // For voltage as 0-500V (0-50000)
    const SCALE_CURRENT: f32 = 100.0; // For current as -300-300A
    const SCALE_TEMP: f32 = 100.0; // For temp as -50-50C

    pub fn new(id: String, can_interface: &str) -> Result<Self, io::Error> {
        let mut can_driver = CanDriver::new(can_interface);
        can_driver.connect().map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
        Ok(Self {
            id,
            can_driver: Some(can_driver),
            ..Default::default()
        })
    }

    /// Pack BatteryStatus into CAN frame data
    /// Format: soc(u8), voltage(u16), current(i16), temp(i16), sop_charge(u16), sop_discharge(u16)
    /// Total: 11 bytes, but clamped to 8 bytes by using scaled values
    fn pack_battery_status(status: &BatteryStatus) -> Vec<u8> {
        // Clamp values to expected ranges to prevent overflow
        let soc = (status.soc.clamp(0.0, 100.0) * Self::SCALE_PERCENT) as u8; // Change to u8 to save space
        let voltage = (status.voltage.clamp(0.0, 500.0) * Self::SCALE_VOLTAGE) as u16;
        let current = (status.current.clamp(-300.0, 300.0) * Self::SCALE_CURRENT) as i16;
        let temp = (status.temperature.clamp(-50.0, 50.0) * Self::SCALE_TEMP) as i16;
        let sop_charge = (status.sop_charge.clamp(0.0, 100.0) * Self::SCALE_PERCENT) as u16;
        let sop_discharge = (status.sop_discharge.clamp(0.0, 100.0) * Self::SCALE_PERCENT) as u16;

        let mut data = Vec::with_capacity(11);
        data.push(soc);
        data.extend_from_slice(&voltage.to_be_bytes());
        data.extend_from_slice(&current.to_be_bytes());
        data.extend_from_slice(&temp.to_be_bytes());
        data.extend_from_slice(&sop_charge.to_be_bytes());
        data.extend_from_slice(&sop_discharge.to_be_bytes());
        data
    }

    /// Unpack CAN frame data into BatteryStatus
    /// Expects at least 11 bytes: soc(u8), voltage(u16), current(i16), temp(i16), sop_charge(u16), sop_discharge(u16)
    fn unpack_battery_status(data: &[u8]) -> BatteryStatus {
        if data.len() < 11 {
            return BatteryStatus::default();
        }
        let soc = data[0] as u16;
        let voltage = u16::from_be_bytes([data[1], data[2]]);
        let current = i16::from_be_bytes([data[3], data[4]]);
        let temp = i16::from_be_bytes([data[5], data[6]]);
        let sop_charge = u16::from_be_bytes([data[7], data[8]]);
        let sop_discharge = u16::from_be_bytes([data[9], data[10]]);
        BatteryStatus {
            soc: soc as f32 / Self::SCALE_PERCENT,
            voltage: voltage as f32 / Self::SCALE_VOLTAGE,
            current: current as f32 / Self::SCALE_CURRENT,
            temperature: temp as f32 / Self::SCALE_TEMP,
            sop_charge: sop_charge as f32 / Self::SCALE_PERCENT,
            sop_discharge: sop_discharge as f32 / Self::SCALE_PERCENT,
        }
    }

    pub fn read_status(&self) -> Result<BatteryStatus, io::Error> {
        if let Some(driver) = &self.can_driver {
            // Send read request
            let request_frame = CanFrame::Data(CanDataFrame::new(StandardId::new(0x100).unwrap(), &[0x01, 0, 0, 0, 0, 0, 0, 0]).unwrap());
            driver.send_frame(&request_frame).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            // Receive response (in real impl, might need timeout/loop)
            let response_frame = driver.recv_frame().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            match response_frame {
                CanFrame::Data(data_frame) if data_frame.id() == Id::Standard(StandardId::new(0x101).unwrap()) => {
                    Ok(Self::unpack_battery_status(&data_frame.data()[..data_frame.dlc() as usize]))
                }
                CanFrame::Data(_) => Err(io::Error::new(io::ErrorKind::InvalidData, "Unexpected CAN frame ID")),
                _ => Err(io::Error::new(io::ErrorKind::InvalidData, "Expected data frame")),
            }
        } else {
            Err(io::Error::new(io::ErrorKind::NotConnected, "CAN driver not initialized"))
        }
    }

    pub fn write_status(&self, status: BatteryStatus) -> Result<(), io::Error> {
        if let Some(driver) = &self.can_driver {
            let data = Self::pack_battery_status(&status);
            if data.len() > 8 {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "Packed data exceeds CAN frame size limit"));
            }
            let frame = CanFrame::Data(CanDataFrame::new(StandardId::new(0x102).unwrap(), &data).unwrap());
            driver.send_frame(&frame).map_err(|e| io::Error::new(io::ErrorKind::Other, e))
        } else {
            Err(io::Error::new(io::ErrorKind::NotConnected, "CAN driver not initialized"))
        }
    }

    // Additional scaling for cell data
    const SCALE_CELL_VOLTAGE: f32 = 10000.0; // Cell voltage in 0.0001V units
    const SCALE_CELL_TEMP: f32 = 100.0;      // Cell temp in 0.01°C units

    /// Read cell status information from the battery device
    ///
    /// # Returns
    /// Result containing BatteryCellStatus or IO error
    pub fn read_cell_status(&self) -> Result<BatteryCellStatus, io::Error> {
        if let Some(driver) = &self.can_driver {
            // Send read request for cell status (assume CAN ID 0x103)
            let request_frame = CanFrame::Data(CanDataFrame::new(StandardId::new(0x103).unwrap(), &[0x01]).unwrap());
            driver.send_frame(&request_frame).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

            // Receive response
            let response_frame = driver.recv_frame().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
            match response_frame {
                CanFrame::Data(data_frame) if data_frame.id() == Id::Standard(StandardId::new(0x104).unwrap()) => {
                    Ok(Self::unpack_battery_cell_status(&data_frame.data()[..data_frame.dlc() as usize]))
                }
                CanFrame::Data(_) => Err(io::Error::new(io::ErrorKind::InvalidData, "Unexpected CAN frame ID for cell status")),
                _ => Err(io::Error::new(io::ErrorKind::InvalidData, "Expected data frame for cell status")),
            }
        } else {
            Err(io::Error::new(io::ErrorKind::NotConnected, "CAN driver not initialized"))
        }
    }

    /// Pack BatteryCellStatus into CAN frame data
    fn pack_battery_cell_status(status: &BatteryCellStatus) -> Vec<u8> {
        // Simplified packing
        let mut data = Vec::with_capacity(16);
        let cell_count = status.cell_count;
        let max_v = (status.max_cell_voltage * Self::SCALE_CELL_VOLTAGE) as u16;
        let min_v = (status.min_cell_voltage * Self::SCALE_CELL_VOLTAGE) as u16;
        let max_t = ((status.max_cell_temperature + 50.0) * Self::SCALE_CELL_TEMP) as u16;
        let min_t = ((status.min_cell_temperature + 50.0) * Self::SCALE_CELL_TEMP) as u16;
        let working_time = status.working_time;
        let cycle_count = status.cycle_count;
        let health = (status.health_percentage * Self::SCALE_PERCENT) as u16;

        data.extend_from_slice(&cell_count.to_be_bytes());
        data.extend_from_slice(&max_v.to_be_bytes());
        data.extend_from_slice(&min_v.to_be_bytes());
        data.extend_from_slice(&max_t.to_be_bytes());
        data.extend_from_slice(&min_t.to_be_bytes());
        data.extend_from_slice(&working_time.to_be_bytes());
        data.extend_from_slice(&cycle_count.to_be_bytes());
        data.extend_from_slice(&health.to_be_bytes());
        data
    }

    /// Unpack CAN frame data into BatteryCellStatus
    fn unpack_battery_cell_status(data: &[u8]) -> BatteryCellStatus {
        if data.len() < 18 {
            return BatteryCellStatus::default();
        }
        let cell_count = u16::from_be_bytes([data[0], data[1]]);
        let max_v = u16::from_be_bytes([data[2], data[3]]) as f32 / Self::SCALE_CELL_VOLTAGE;
        let min_v = u16::from_be_bytes([data[4], data[5]]) as f32 / Self::SCALE_CELL_VOLTAGE;
        let max_t = u16::from_be_bytes([data[6], data[7]]) as f32 / Self::SCALE_CELL_TEMP - 50.0;
        let min_t = u16::from_be_bytes([data[8], data[9]]) as f32 / Self::SCALE_CELL_TEMP - 50.0;
        let working_time = u32::from_be_bytes([data[10], data[11], data[12], data[13]]);
        let cycle_count = u16::from_be_bytes([data[14], data[15]]);
        let health = u16::from_be_bytes([data[16], data[17]]) as f32 / Self::SCALE_PERCENT;

        BatteryCellStatus {
            cell_count,
            max_cell_voltage: max_v,
            min_cell_voltage: min_v,
            max_cell_temperature: max_t,
            min_cell_temperature: min_t,
            working_time,
            cycle_count,
            health_percentage: health,
        }
    }
}
