// 核心 EMS 控制逻辑
// Energy Management System controller implementing power balancing between PV, battery, generator, and chargers

use crate::devices::*;
use crate::types::*;
use std::io;
use std::sync::{Arc, Mutex};
use log;

/// Configuration for EMS operation
#[derive(Debug, Clone)]
pub struct EmsConfig {
    /// Battery SOC threshold to start generator (0-100%)
    pub battery_soc_threshold: f32,
    /// Maximum charger power per station in kW
    pub max_charger_power: f32,
    /// Total number of charging stations
    pub num_charging_stations: usize,
    /// Control loop interval in seconds
    pub control_interval: u64,
}

/// EMS operational modes
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmsMode {
    Normal,      // Normal operation with power balancing
    PeakShaving, // Reducing peak demand
    Emergency,   // Emergency generator-only operation
    Fault,       // System fault condition
}

/// Energy Management System Controller
#[derive(Debug)]
pub struct EmsController {
/// PV generation devices
pv_devices: Vec<Arc<Mutex<PvDevice>>>,
/// Battery management system
battery_device: Option<Arc<Mutex<BatteryDevice>>>,
/// Generator set
genset_device: Option<Arc<Mutex<GensetDevice>>>,
/// Power conversion system
pcs_device: Option<Arc<Mutex<PcsDevice>>>,
/// Charging stations (vector of charger devices)
charger_devices: Vec<Arc<Mutex<ChargerDevice>>>,
/// EMS configuration
config: EmsConfig,
/// Current operational mode
current_mode: EmsMode,
/// Cached system status
cached_status: std::cell::RefCell<EmsStatus>,
/// Control loop running flag
running: bool,
}

impl EmsController {
    /// Default configuration values
    const DEFAULT_CONFIG: EmsConfig = EmsConfig {
        battery_soc_threshold: 20.0, // Start generator when battery SOC < 20%
        max_charger_power: 22.0,     // 22kW per charger (common EV charger rating)
        num_charging_stations: 15,   // 15 charging stations total
        control_interval: 5,         // 5 second control loop
    };

    /// Create a new EMS controller with default configuration
    ///
    /// # Returns
    /// Result containing the EMS controller or initialization error
    pub fn new() -> Result<Self, String> {
        Self::with_config(Self::DEFAULT_CONFIG)
    }

    /// Create a new EMS controller with custom configuration
    ///
    /// # Arguments
    /// * `config` - EMS configuration parameters
    ///
    /// # Returns
    /// Result containing the EMS controller or initialization error
    pub fn with_config(config: EmsConfig) -> Result<Self, String> {
        // Charger devices are added dynamically using add_charger_device()

        Ok(Self {
            pv_devices: Vec::new(), // PV devices are added dynamically using add_pv_device()
            battery_device: None, // TODO: Initialize battery device
            genset_device: None, // TODO: Initialize genset device
            pcs_device: None, // TODO: Initialize PCS device
            charger_devices: Vec::new(), // Charger devices are added dynamically
            config,
            current_mode: EmsMode::Normal,
            cached_status: std::cell::RefCell::new(EmsStatus::default()),
            running: false,
        })
    }

    /// Add PV device to the EMS
    ///
    /// # Arguments
    /// * `device` - Initialized PV device
    ///
    /// # Returns
    /// Result indicating success or error message
    pub fn add_pv_device(&mut self, device: Arc<Mutex<PvDevice>>) -> Result<(), String> {
        self.pv_devices.push(device);
        Ok(())
    }

    /// Add battery device to the EMS
    ///
    /// # Arguments
    /// * `device` - Initialized battery device
    pub fn add_battery_device(&mut self, device: Arc<Mutex<BatteryDevice>>) {
        self.battery_device = Some(device);
    }

    /// Add generator device to the EMS
    ///
    /// # Arguments
    /// * `device` - Initialized generator device
    pub fn add_genset_device(&mut self, device: Arc<Mutex<GensetDevice>>) {
        self.genset_device = Some(device);
    }

    /// Add PCS device to the EMS
    ///
    /// # Arguments
    /// * `device` - Initialized PCS device
    pub fn add_pcs_device(&mut self, device: Arc<Mutex<PcsDevice>>) {
        self.pcs_device = Some(device);
    }

    /// Add charger device to the EMS
    ///
    /// # Arguments
    /// * `device` - Initialized charger device
    ///
    /// # Returns
    /// Result indicating success or error message
    pub fn add_charger_device(&mut self, device: Arc<Mutex<ChargerDevice>>) -> Result<(), String> {
        let id = {
            let dev = device.lock().map_err(|_| "Mutex poisoned".to_string())?;
            dev.id.clone()
        };
        // Check if device with same ID already exists
        for c in &self.charger_devices {
            let existing_id = c.lock().map_err(|_| "Mutex poisoned".to_string())?.id.clone();
            if existing_id == id {
                return Err(format!("Charger device with ID '{}' already exists", id));
            }
        }
        self.charger_devices.push(device);
        Ok(())
    }

    /// Remove charger device from the EMS
    ///
    /// # Arguments
    /// * `id` - Device ID to remove
    ///
    /// # Returns
    /// Result indicating success or error message
    pub fn remove_charger_device(&mut self, id: &str) -> Result<(), String> {
        let initial_len = self.charger_devices.len();
        self.charger_devices.retain(|c| c.id != id);
        if self.charger_devices.len() == initial_len {
            return Err(format!("Charger device with ID '{}' not found", id));
        }
        Ok(())
    }

    /// Start the EMS control loop
    ///
    /// # Returns
    /// Result indicating success or startup error
    pub fn start(&mut self) -> Result<(), String> {
        if self.running {
            return Err("EMS is already running".to_string());
        }

        self.running = true;
        self.current_mode = EmsMode::Normal;
        Ok(())
    }

    /// Stop the EMS control loop
    pub fn stop(&mut self) {
        self.running = false;
        self.current_mode = EmsMode::Fault;
    }

    /// Execute one control cycle
    ///
    /// # Returns
    /// Result indicating success or control error
    pub fn run_control_cycle(&mut self) -> Result<(), String> {
        if !self.running {
            return Ok(()); // Skip if not running
        }

        // 1. Read all device statuses
        let (pv_power, battery_soc, battery_power, generator_power) = self.read_device_statuses()?;

        // 2. Calculate total available power
        let available_power = pv_power + generator_power;

        // 3. Calculate current charger demand
        let charger_demand = self.calculate_charger_demand();

        // 4. Execute power balancing logic
        self.balance_power(available_power, charger_demand, battery_soc)?;

        // 5. Update cached status
        self.update_cached_status(pv_power, battery_power, generator_power, charger_demand);

        Ok(())
    }

    /// Read status from all connected devices
    ///
    /// # Returns
    /// Tuple of (pv_power, battery_soc, battery_power, generator_power) or error
    fn read_device_statuses(&mut self) -> Result<(f32, f32, f32, f32), String> {
        let mut pv_power = 0.0;
        let mut battery_soc = 100.0; // Default to full if no battery
        let mut battery_power = 0.0;
        let mut generator_power = 0.0;

        // Read PV status from all devices
        for pv in &self.pv_devices {
            let mut pv_locked = pv.lock().map_err(|_| "Mutex poisoned".to_string())?;
            match pv_locked.read_status() {
                Ok(status) => pv_power += status.power / 1000.0, // Convert W to kW and accumulate
                Err(e) => log::warn!("Failed to read PV status for device {}: {}", pv_locked.id, e),
            }
        }

        // Read battery status
        if let Some(ref battery) = self.battery_device {
            let battery_locked = battery.lock().map_err(|_| "Mutex poisoned".to_string())?;
            match battery_locked.read_status() {
                Ok(status) => {
                    battery_soc = status.soc;
                    battery_power = status.current * status.voltage / 1000.0; // Convert W to kW
                }
                Err(e) => log::warn!("Failed to read battery status: {}", e),
            }
        }

        // Read generator status
        if let Some(ref genset) = self.genset_device {
            let mut genset_locked = genset.lock().map_err(|_| "Mutex poisoned".to_string())?;
            match genset_locked.read_status() {
                Ok(status) => {
                    generator_power = if status.running { status.power_output } else { 0.0 };
                }
                Err(e) => log::warn!("Failed to read generator status: {}", e),
            }
        }

        Ok((pv_power, battery_soc, battery_power, generator_power))
    }

    /// Calculate total power demand from all charging stations
    ///
    /// # Returns
    /// Total charger power demand in kW
    fn calculate_charger_demand(&self) -> f32 {
        let mut total_demand = 0.0;

        for charger in &self.charger_devices {
            let charger_locked = match charger.lock() {
                Ok(c) => c,
                Err(_) => continue,
            };
            let status = charger_locked.get_cached_status();
            if status.charging {
                total_demand += status.power;
            }
        }

        total_demand
    }

    /// Execute power balancing logic according to priority: PV > Grid > Battery > Generator
    ///
    /// # Arguments
    /// * `available_power` - Available power from PV and generator in kW
    /// * `charger_demand` - Current charger power demand in kW
    /// * `battery_soc` - Current battery state of charge (0-100%)
    ///
    /// # Returns
    /// Result indicating success or balancing error
    fn balance_power(&mut self, available_power: f32, charger_demand: f32, battery_soc: f32) -> Result<(), String> {
        let power_deficit = charger_demand - available_power;

        if power_deficit <= 0.0 {
            // Surplus power available
            let surplus = -power_deficit;

            // Priority 1: Use surplus to charge battery if SOC is low
            if battery_soc < 90.0 && surplus > 0.0 {
                self.charge_battery(surplus.min(50.0))?; // Limit charging rate
            }

            // Priority 2: Export to grid (not implemented yet)
            // TODO: Implement grid export logic

        } else {
            // Power deficit - need additional sources
            let mut remaining_deficit = power_deficit;

            // Priority 1: Discharge battery if SOC is sufficient
            if battery_soc > self.config.battery_soc_threshold + 5.0 && remaining_deficit > 0.0 {
                let battery_contribution = remaining_deficit.min(50.0); // Limit discharge rate
                self.discharge_battery(battery_contribution)?;
                remaining_deficit -= battery_contribution;
            }

            // Priority 2: Start generator if battery SOC is low and deficit remains
            if battery_soc <= self.config.battery_soc_threshold && remaining_deficit > 0.0 {
                self.start_generator()?;
                // Assume generator can provide remaining deficit
                // In real implementation, would need to check generator capacity
            }

            // Priority 3: Reduce charger power if still insufficient
            if remaining_deficit > 0.0 {
                self.reduce_charger_power(charger_demand - remaining_deficit)?;
            }
        }

        Ok(())
    }

    /// Charge battery with specified power
    ///
    /// # Arguments
    /// * `power` - Charging power in kW
    ///
    /// # Returns
    /// Result indicating success or battery control error
    fn charge_battery(&mut self, power: f32) -> Result<(), String> {
        if let Some(ref pcs) = self.pcs_device {
            let mut pcs_locked = pcs.lock().map_err(|_| "Mutex poisoned".to_string())?;
            pcs_locked.set_mode(crate::devices::pcs::PcsMode::Charging)
                .map_err(|e| format!("Failed to set PCS charging mode: {:?}", e))?;
            pcs_locked.set_power_setpoint(-power) // Negative for charging
                .map_err(|e| format!("Failed to set PCS charging power: {:?}", e))?;
        }
        Ok(())
    }

    /// Discharge battery with specified power
    ///
    /// # Arguments
    /// * `power` - Discharging power in kW
    ///
    /// # Returns
    /// Result indicating success or battery control error
    fn discharge_battery(&mut self, power: f32) -> Result<(), String> {
        if let Some(ref pcs) = self.pcs_device {
            let mut pcs_locked = pcs.lock().map_err(|_| "Mutex poisoned".to_string())?;
            pcs_locked.set_mode(crate::devices::pcs::PcsMode::Discharging)
                .map_err(|e| format!("Failed to set PCS discharging mode: {:?}", e))?;
            pcs_locked.set_power_setpoint(power)
                .map_err(|e| format!("Failed to set PCS discharging power: {:?}", e))?;
        }
        Ok(())
    }

    /// Start the generator
    ///
    /// # Returns
    /// Result indicating success or generator start error
    fn start_generator(&mut self) -> Result<(), String> {
        if let Some(ref genset) = self.genset_device {
            let mut genset_locked = genset.lock().map_err(|_| "Mutex poisoned".to_string())?;
            genset_locked.start_engine()
                .map_err(|e| format!("Failed to start generator: {:?}", e))?;
        }
        Ok(())
    }

    /// Reduce total charger power to match available power
    ///
    /// # Arguments
    /// * `max_power` - Maximum allowed total charger power in kW
    ///
    /// # Returns
    /// Result indicating success or charger control error
    fn reduce_charger_power(&mut self, max_power: f32) -> Result<(), String> {
        let active_chargers = self.charger_devices.iter()
            .filter(|c| {
                if let Ok(charger_locked) = c.lock() {
                    charger_locked.get_cached_status().charging
                } else {
                    false
                }
            })
            .count();

        if active_chargers == 0 {
            return Ok(());
        }

        let power_per_charger = (max_power / active_chargers as f32).min(self.config.max_charger_power);

        for charger in &self.charger_devices {
            let mut charger_locked = charger.lock().map_err(|_| "Mutex poisoned".to_string())?;
            if charger_locked.get_cached_status().charging {
                charger_locked.set_power_setpoint(power_per_charger)
                    .map_err(|e| format!("Failed to set charger power: {}", e))?;
            }
        }

        Ok(())
    }

    /// Update cached system status
    ///
    /// # Arguments
    /// * `pv_power` - Current PV power generation in kW
    /// * `battery_power` - Current battery power flow in kW
    /// * `generator_power` - Current generator power output in kW
    /// * `charger_power` - Current charger power consumption in kW
    fn update_cached_status(&self, pv_power: f32, battery_power: f32, generator_power: f32, charger_power: f32) {
        let total_generation = pv_power + generator_power;
        let total_consumption = charger_power;
        let power_balance = total_generation - total_consumption;

        let active_chargers = self.charger_devices.iter()
            .filter(|c| {
                if let Ok(charger_locked) = c.lock() {
                    charger_locked.get_cached_status().charging
                } else {
                    false
                }
            })
            .count();

        if let Ok(mut status) = self.cached_status.try_borrow_mut() {
            *status = EmsStatus {
                total_generation,
                total_consumption,
                power_balance,
                grid_power: 0.0, // TODO: Implement grid power monitoring
                battery_power,
                generator_power,
                pv_power,
                charger_power,
                active_chargers,
                system_mode: format!("{:?}", self.current_mode),
                system_healthy: true, // TODO: Implement health monitoring
                faults: vec![], // TODO: Collect system faults
            };
        }
    }

    /// Get current system status
    ///
    /// # Returns
    /// Current EMS system status
    pub fn get_status(&self) -> EmsStatus {
        self.cached_status.borrow().clone()
    }

    /// Check if EMS is currently running
    ///
    /// # Returns
    /// True if control loop is running, false otherwise
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Get current operational mode
    ///
    /// # Returns
    /// Current EMS operational mode
    pub fn get_mode(&self) -> &EmsMode {
        &self.current_mode
    }
}
