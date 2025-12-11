// 后端类型定义
// Placeholder: Rust 数据结构

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EmsStatus {
    /// Total power generation (PV + Generator) in kW
    pub total_generation: f32,
    /// Total power consumption (chargers + system load) in kW
    pub total_consumption: f32,
    /// Net power balance (generation - consumption) in kW
    pub power_balance: f32,
    /// Grid power usage in kW (positive: importing, negative: exporting)
    pub grid_power: f32,
    /// Battery power flow in kW (positive: discharging, negative: charging)
    pub battery_power: f32,
    /// Generator power output in kW
    pub generator_power: f32,
    /// PV power generation in kW
    pub pv_power: f32,
    /// Total charger power consumption in kW
    pub charger_power: f32,
    /// Number of active charging stations
    pub active_chargers: usize,
    /// System operational mode
    pub system_mode: String,
    /// System health status
    pub system_healthy: bool,
    /// Active system faults/warnings
    pub faults: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PvStatus {
    /// DC voltage output in V
    pub voltage: f32,
    /// DC current output in A
    pub current: f32,
    /// DC power output in W
    pub power: f32,
    /// Operating temperature in °C
    pub temperature: f32,
    /// Efficiency percentage (0-100%)
    pub efficiency: f32,
    /// Fault status
    pub fault: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BatteryStatus {
    pub soc: f32,
    pub voltage: f32,
    pub current: f32,
    pub temperature: f32,
    pub sop_charge: f32,
    pub sop_discharge: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BatteryCellStatus {
    pub cell_count: u16,
    pub max_cell_voltage: f32,
    pub min_cell_voltage: f32,
    pub max_cell_temperature: f32,
    pub min_cell_temperature: f32,
    pub working_time: u32,
    pub cycle_count: u16,
    pub health_percentage: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GensetStatus {
    pub running: bool,
    pub power_output: f32,
    pub fuel_level: f32,
    pub voltage: f32,
    pub current: f32,
    pub frequency: f32,
    pub engine_hours: u32,
    pub temperature: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChargerStatus {
    /// Whether the charger is currently charging
    pub charging: bool,
    /// Charging power in kW
    pub power: f32,
    /// Output voltage in V
    pub voltage: f32,
    /// Output current in A
    pub current: f32,
    /// Operating temperature in °C
    pub temperature: f32,
    /// Efficiency percentage (0-100%)
    pub efficiency: f32,
    /// Fault status
    pub fault: bool,
    /// Active fault codes
    pub fault_codes: Vec<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PcsStatus {
    pub mode: String,    // Operating mode (e.g., "Charging", "Discharging", "Standby")
    pub power: f32,      // Active power in kW
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GpsData {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f32,
    pub speed: f32,
    pub timestamp: String,
}
