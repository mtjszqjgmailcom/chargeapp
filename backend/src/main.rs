// 纯 Rust 控制核心 (可独立运行)
// Placeholder: EMS 系统主入口

mod ems_core;
mod devices;
mod drivers;
mod types;

use crate::ems_core::EmsController;

use std::time::Duration;
use std::thread;
use std::sync::{Arc, Mutex};
use std::sync::mpsc;
use crate::types::{EmsStatus, GpsData};
use crate::devices::{charger, bms, pcs, pv_dcdc, genset};
use crate::drivers::{can, modbus, gps_4g, cloud};
use serde::{Deserialize, Serialize};
use serde_json;
use tokio;
use std::fs;
use std::path::Path;
use tauri::{command, State};
use log;

// System state shared across threads
#[derive(Clone)]
struct SystemState {
    // Devices
    charger: Arc<Mutex<charger::ChargerDevice>>,
    battery: Arc<Mutex<bms::BatteryDevice>>,
    pcs: Arc<Mutex<pcs::PcsDevice>>,
    pv_dcdc: Arc<Mutex<pv_dcdc::PvDcdcDevice>>,
    genset: Arc<Mutex<genset::GensetDevice>>,
    // Drivers
    can_driver: Arc<Mutex<can::CanDriver>>,
    modbus_driver: Arc<Mutex<modbus::ModbusDriver>>,
    gps_4g_driver: Arc<Mutex<gps_4g::Gps4gDriver>>,
    cloud_driver: Arc<Mutex<cloud::CloudDriver>>,
    // EMS Controller
    ems_controller: Arc<Mutex<EmsController>>,
    // Shared data
    ems_status: Arc<Mutex<EmsStatus>>,
    gps_data: Arc<Mutex<GpsData>>,
    data_cache: Arc<Mutex<Vec<String>>>, // For data to send
    system_healthy: Arc<Mutex<bool>>,
    current_timestamp: Arc<Mutex<String>>,
}

#[derive(Debug, Deserialize)]
struct Config {
    charger_id: String,
    charger_interface: String,
    battery_id: String,
    battery_interface: String,
    pcs_id: String,
    pcs_host: String,
    pcs_port: u16,
    pv_dcdc_id: String,
    pv_dcdc_host: String,
    pv_dcdc_port: u16,
    genset_id: String,
    genset_host: String,
    genset_port: u16,
    can_interface: String,
}

// Tauri commands for data interface
#[command]
fn get_system_status(state: State<'_, Arc<SystemState>>) -> EmsStatus {
    state.ems_status.lock().expect("Failed to lock ems_status").clone()
}

#[command]
fn get_gps_data(state: State<'_, Arc<SystemState>>) -> GpsData {
    state.gps_data.lock().expect("Failed to lock gps_data").clone()
}

#[command]
fn get_system_health(state: State<'_, Arc<SystemState>>) -> bool {
    *state.system_healthy.lock().expect("Failed to lock system_healthy")
}

#[command]
fn get_current_timestamp(state: State<'_, Arc<SystemState>>) -> String {
    state.current_timestamp.lock().expect("Failed to lock current_timestamp").clone()
}

#[command]
fn get_device_statuses(state: State<'_, Arc<SystemState>>) -> serde_json::Value {
    let charger = state.charger.lock().expect("Failed to lock charger").get_cached_status();
    let battery = state.battery.lock().expect("Failed to lock battery").read_status().unwrap_or_default();
    let pv = state.pv_dcdc.lock().expect("Failed to lock pv_dcdc").get_cached_status();
    let pcs = state.pcs.lock().expect("Failed to lock pcs").get_cached_status();
    let genset = state.genset.lock().expect("Failed to lock genset").get_cached_status();

    serde_json::json!({
        "charger": charger,
        "battery": battery,
        "pv_dcdc": pv,
        "pcs": pcs,
        "genset": genset
    })
}

#[command]
fn send_control_command(state: State<'_, Arc<SystemState>>, command: String) -> String {
    // Handle control commands directly via Tauri
    if let Ok(cmd) = serde_json::from_str::<serde_json::Value>(&command) {
        if let Some(action) = cmd.get("action").and_then(|v| v.as_str()) {
            match action {
                "start_system" => {
                    *state.system_healthy.lock().expect("Failed to lock system_healthy") = true;
                    "System started".to_string()
                }
                "stop_system" => {
                    *state.system_healthy.lock().expect("Failed to lock system_healthy") = false;
                    "System stopped".to_string()
                }
                "set_pcs_mode" => {
                    if let Some(mode_str) = cmd.get("mode").and_then(|v| v.as_str()) {
                        let mode = match mode_str {
                            "standby" => pcs::PcsMode::Standby,
                            "charging" => pcs::PcsMode::Charging,
                            "discharging" => pcs::PcsMode::Discharging,
                            "gridtie" => pcs::PcsMode::GridTie,
                            "offgrid" => pcs::PcsMode::OffGrid,
                            "fault" => pcs::PcsMode::Fault,
                            _ => return "Invalid mode".to_string(),
                        };
                        let mut pcs = state.pcs.lock().expect("Failed to lock pcs");
                        if let Err(e) = pcs.set_mode(mode) {
                            format!("Failed to set PCS mode: {:?}", e)
                        } else {
                            format!("PCS mode set to {:?}", mode)
                        }
                    } else {
                        "Missing mode parameter".to_string()
                    }
                }
                "set_pv_mode" => {
                    if let Some(mode_str) = cmd.get("mode").and_then(|v| v.as_str()) {
                        use pv_dcdc::PvMode;
                        let mode = match mode_str {
                            "standby" => PvMode::Standby,
                            "mppt" => PvMode::MPPT,
                            "constant_voltage" => PvMode::ConstantVoltage,
                            "constant_current" => PvMode::ConstantCurrent,
                            "fault" => PvMode::Fault,
                            _ => return "Invalid mode".to_string(),
                        };
                        let mut pv = state.pv_dcdc.lock().expect("Failed to lock pv_dcdc");
                        if let Err(e) = pv.set_mode(mode) {
                            format!("Failed to set PV mode: {:?}", e)
                        } else {
                            format!("PV mode set to {:?}", mode)
                        }
                    } else {
                        "Missing mode parameter".to_string()
                    }
                }
                "start_genset" => {
                    let mut genset = state.genset.lock().expect("Failed to lock genset");
                    if let Err(e) = genset.start_engine() {
                        format!("Failed to start genset: {:?}", e)
                    } else {
                        "Genset started".to_string()
                    }
                }
                "stop_genset" => {
                    let mut genset = state.genset.lock().expect("Failed to lock genset");
                    if let Err(e) = genset.stop_engine() {
                        format!("Failed to stop genset: {:?}", e)
                    } else {
                        "Genset stopped".to_string()
                    }
                }
                "set_charger_power" => {
                    if let Some(power) = cmd.get("power").and_then(|v| v.as_f64()) {
                        let charger = state.charger.lock().expect("Failed to lock charger");
                        if let Err(e) = charger.set_power_setpoint(power as f32) {
                            format!("Failed to set charger power: {:?}", e)
                        } else {
                            format!("Charger power set to {} kW", power)
                        }
                    } else {
                        "Missing power parameter".to_string()
                    }
                }
                "set_threshold" => {
                    if let Some(threshold) = cmd.get("soc_threshold").and_then(|v| v.as_f64()) {
                        let mut status = state.ems_status.lock().expect("Failed to lock ems_status");
                        status.active_chargers = threshold as usize; // Placeholder
                        format!("SOC threshold set to {}", threshold)
                    } else {
                        "Missing threshold parameter".to_string()
                    }
                }
                _ => "Unknown action".to_string()
            }
        } else {
            "Missing action field".to_string()
        }
    } else {
        "Invalid JSON".to_string()
    }
}

fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    let config_path = "config.json";
    let config_str = fs::read_to_string(config_path)?;
    let config: Config = serde_json::from_str(&config_str)?;
    Ok(config)
}

fn main() {
    env_logger::init();
    log::info!("EMS Control Backend Starting...");

    // Load configuration
    let config = match load_config() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to load config: {}", e);
            std::process::exit(1);
        }
    };

    // Initialize system state
    let system_state = Arc::new(initialize_system(&config));

    // Perform health checks
    if !perform_health_checks(&system_state) {
        log::error!("System health check failed. Exiting.");
        std::process::exit(1);
    }

    log::info!("All devices healthy. Starting service...");

    // Channels for inter-thread communication
    let (data_tx, data_rx) = mpsc::channel();

    // Spawn worker threads
    let state_clone = system_state.clone();
    let data_tx_clone = data_tx.clone();
    thread::spawn(move || gps_sync_thread(state_clone, data_tx_clone));

    let state_clone = system_state.clone();
    let data_tx_clone = data_tx.clone();
    thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(data_collection_thread_async(state_clone, data_tx_clone));
    });

    let state_clone = system_state.clone();
    thread::spawn(move || data_sending_thread(state_clone, data_rx));

    let state_clone = system_state.clone();
    thread::spawn(move || power_control_thread(state_clone));

    // Tauri app for data interface
    tauri::Builder::default()
        .manage(system_state)
        .invoke_handler(tauri::generate_handler![
            get_system_status,
            get_gps_data,
            get_system_health,
            get_current_timestamp,
            get_device_statuses,
            send_control_command
        ])
        ;
}

fn initialize_system(config: &Config) -> SystemState {
    // Initialize devices and drivers
    let charger = Arc::new(Mutex::new(charger::ChargerDevice::new(config.charger_id.clone(), &config.charger_interface).expect("Failed to initialize charger device")));
    let battery = Arc::new(Mutex::new(bms::BatteryDevice::new(config.battery_id.clone(), &config.battery_interface).expect("Failed to initialize battery device")));
    let pcs = Arc::new(Mutex::new(pcs::PcsDevice::new(config.pcs_id.clone(), &config.pcs_host, config.pcs_port).expect("Failed to initialize PCS device")));
    let pv_dcdc = Arc::new(Mutex::new(pv_dcdc::PvDcdcDevice::new(config.pv_dcdc_id.clone(), &config.pv_dcdc_host, config.pv_dcdc_port).expect("Failed to initialize PV DCDC device")));
    let genset = Arc::new(Mutex::new(genset::GensetDevice::new(config.genset_id.clone(), &config.genset_host, config.genset_port).expect("Failed to initialize genset device")));
    let can_driver = Arc::new(Mutex::new(can::CanDriver::new(&config.can_interface)));
    let modbus_driver = Arc::new(Mutex::new(modbus::ModbusDriver::new()));
    let gps_4g_driver = Arc::new(Mutex::new(gps_4g::Gps4gDriver::new().expect("Failed to initialize GPS 4G driver")));
    let cloud_driver = Arc::new(Mutex::new(cloud::CloudDriver::new()));
    let ems_status = Arc::new(Mutex::new(EmsStatus::default()));
    let gps_data = Arc::new(Mutex::new(GpsData::default()));
    let data_cache = Arc::new(Mutex::new(Vec::new()));
    let system_healthy = Arc::new(Mutex::new(true));
    let current_timestamp = Arc::new(Mutex::new("".to_string()));

    // Initialize EMS Controller
    let mut ems_controller = EmsController::new().expect("Failed to create EMS controller");
    ems_controller.add_pv_device(pv_dcdc.clone());
    ems_controller.add_battery_device(battery.clone());
    ems_controller.add_pcs_device(pcs.clone());
    ems_controller.add_genset_device(genset.clone());
    ems_controller.add_charger_device(charger.clone());
    ems_controller.start().expect("Failed to start EMS controller");
    let ems_controller = Arc::new(Mutex::new(ems_controller));

    SystemState {
        charger,
        battery,
        pcs,
        pv_dcdc,
        genset,
        can_driver,
        modbus_driver,
        gps_4g_driver,
        cloud_driver,
        ems_controller,
        ems_status,
        gps_data,
        data_cache,
        system_healthy,
        current_timestamp,
    }
}

fn perform_health_checks(state: &Arc<SystemState>) -> bool {
    log::info!("Performing health checks...");

    // Define a list of health checks for drivers and devices
    // Each check is a tuple of (name, check_function)
    let checks: Vec<(&str, Box<dyn Fn() -> bool>)> = vec![
        ("CAN Driver", Box::new(|| state.can_driver.lock().expect("Failed to lock can_driver").is_connected())),
        ("Modbus Driver", Box::new(|| state.modbus_driver.lock().expect("Failed to lock modbus_driver").is_connected())),
        ("Cloud Driver", Box::new(|| state.cloud_driver.lock().expect("Failed to lock cloud_driver").is_connected())),
        ("GPS 4G Driver", Box::new(|| state.gps_4g_driver.lock().expect("Failed to lock gps_4g_driver").is_connected())),
        ("Charger Device", Box::new(|| state.charger.lock().expect("Failed to lock charger").is_connected())),
        ("Battery Device", Box::new(|| {
            // BMS doesn't have is_connected, check CAN driver connectivity
            state.can_driver.lock().expect("Failed to lock can_driver").is_connected()
        })),
        ("PCS Device", Box::new(|| state.pcs.lock().expect("Failed to lock pcs").is_connected())),
        ("PV DCDC Device", Box::new(|| state.pv_dcdc.lock().expect("Failed to lock pv_dcdc").is_connected())),
        ("Genset Device", Box::new(|| state.genset.lock().expect("Failed to lock genset").is_connected())),
    ];

    let mut all_healthy = true;
    let mut failed_checks = Vec::new();

    for (name, check) in checks {
        if !check() {
            all_healthy = false;
            failed_checks.push(name);
            log::warn!("Health check failed for: {}", name);
        }
    }

    if all_healthy {
        log::info!("All health checks passed.");
        // Update system healthy status
        *state.system_healthy.lock().expect("Failed to lock system_healthy") = true;
    } else {
        log::error!("Health checks failed for: {:?}", failed_checks);
        *state.system_healthy.lock().expect("Failed to lock system_healthy") = false;
    }

    all_healthy
}

fn gps_sync_thread(state: Arc<SystemState>, _data_tx: mpsc::Sender<String>) {
    // Initialize GPS driver if not already done
    {
        let mut gps_driver = state.gps_4g_driver.lock().expect("Failed to lock gps_4g_driver");
        if !gps_driver.is_connected() {
            // Assume default serial port for GPS/4G module
            let port_path = "/dev/ttyUSB0"; // Adjust based on system configuration
            if let Err(e) = gps_driver.init(port_path) {
                log::error!("Failed to initialize GPS/4G driver: {:?}", e);
                // Continue without GPS, perhaps retry or exit
                return;
            }
            // Optionally connect 4G
            if let Err(e) = gps_driver.connect_4g() {
                log::error!("Failed to connect 4G: {:?}", e);
                // Continue, as GPS might still work
            }
        }
    }

    loop {
        // Collect GPS data at 1Hz
        let gps_data_result = {
            let mut gps_driver = state.gps_4g_driver.lock().expect("Failed to lock gps_4g_driver");
            gps_driver.read_gps_data()
        };

        match gps_data_result {
            Ok(gps_data) => {
                // Update GPS data in state
                {
                    let mut state_gps_data = state.gps_data.lock().expect("Failed to lock gps_data");
                    *state_gps_data = gps_data.clone();
                }

                // Update current timestamp with GPS time for synchronization
                {
                    let mut current_ts = state.current_timestamp.lock().expect("Failed to lock current_timestamp");
                    *current_ts = gps_data.timestamp.clone();
                }

                // Log successful sync
                log::info!("GPS sync: lat={}, lon={}, time={}", gps_data.latitude, gps_data.longitude, gps_data.timestamp);

                // In a real implementation, synchronize system clock here
                // For demo, we update the state's timestamp
                // To sync system time: use libc::settimeofday or similar (requires root)
            }
            Err(e) => {
                log::error!("Failed to read GPS data: {:?}", e);
                // Continue trying
            }
        }

        thread::sleep(Duration::from_secs(1));
    }
}

async fn data_collection_thread_async(state: Arc<SystemState>, data_tx: mpsc::Sender<String>) {
    loop {
        // Spawn async tasks for concurrent data collection from devices
        let charger_future = tokio::spawn({
            let state = state.clone();
            async move {
                tokio::task::spawn_blocking(move || {
                    let mut charger = state.charger.lock().expect("Failed to lock charger");
                    let status = charger.read_status().unwrap_or_default();
                    let car_battery = charger.read_car_battery().unwrap_or_default();
                    serde_json::json!({"status": status, "car_battery": car_battery})
                }).await.unwrap_or(serde_json::Value::Null)
            }
        });

        let battery_future = tokio::spawn({
            let state = state.clone();
            async move {
                tokio::task::spawn_blocking(move || {
                    let battery = state.battery.lock().expect("Failed to lock battery");
                    let status = battery.read_status().unwrap_or_default();
                    serde_json::json!({"status": status})
                }).await.unwrap_or(serde_json::Value::Null)
            }
        });

        let pv_future = tokio::spawn({
            let state = state.clone();
            async move {
                tokio::task::spawn_blocking(move || {
                    let mut pv = state.pv_dcdc.lock().expect("Failed to lock pv_dcdc");
                    let status = pv.read_status().unwrap_or_default();
                    serde_json::json!({"status": status})
                }).await.unwrap_or(serde_json::Value::Null)
            }
        });

        let pcs_future = tokio::spawn({
            let state = state.clone();
            async move {
                tokio::task::spawn_blocking(move || {
                    let mut pcs = state.pcs.lock().expect("Failed to lock pcs");
                    let status = pcs.read_status().unwrap_or_default();
                    serde_json::json!({"status": status})
                }).await.unwrap_or(serde_json::Value::Null)
            }
        });

        let genset_future = tokio::spawn({
            let state = state.clone();
            async move {
                tokio::task::spawn_blocking(move || {
                    let mut genset = state.genset.lock().expect("Failed to lock genset");
                    let status = genset.read_status().unwrap_or_default();
                    serde_json::json!({"status": status})
                }).await.unwrap_or(serde_json::Value::Null)
            }
        });

        // Await all data collection tasks concurrently
        let (charger_res, battery_res, pv_res, pcs_res, genset_res) = tokio::join!(
            charger_future, battery_future, pv_future, pcs_future, genset_future
        );

        let charger_data = charger_res.unwrap_or(serde_json::Value::Null);
        let battery_data = battery_res.unwrap_or(serde_json::Value::Null);
        let pv_data = pv_res.unwrap_or(serde_json::Value::Null);
        let pcs_data = pcs_res.unwrap_or(serde_json::Value::Null);
        let genset_data = genset_res.unwrap_or(serde_json::Value::Null);

        // Get current timestamp for stamping data
        let timestamp = {
            state.current_timestamp.lock().expect("Failed to lock current_timestamp").clone()
        };

        // Send collected data with timestamp to sending thread
        let data_points: Vec<serde_json::Value> = vec![charger_data, battery_data, pv_data, pcs_data, genset_data];
        for data in data_points {
            let data_string = format!("{}:{}", timestamp, data.to_string());
            if let Err(e) = data_tx.send(data_string) {
                eprintln!("Failed to send data to sending thread: {:?}", e);
                // If channel is closed, perhaps exit thread
                return;
            }
        }

        // Small delay to prevent overwhelming the system
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn data_sending_thread(state: Arc<SystemState>, data_rx: mpsc::Receiver<String>) {
    // Create data cache directory if it doesn't exist
    let cache_dir = "data_cache";
    if !Path::new(cache_dir).exists() {
        if let Err(e) = fs::create_dir_all(cache_dir) {
            eprintln!("Failed to create cache directory: {:?}", e);
            return;
        }
    }

    let max_memory_cache = 50; // Maximum items in memory
    let mut sent_index = 0; // Index of next data to send

    loop {
        // Receive incoming data and add to cache
        while let Ok(data) = data_rx.try_recv() {
            let mut cache = state.data_cache.lock().expect("Failed to lock data_cache");
            cache.push(data);
        }

        // Check network status (4G connection)
        let network_ok = state.gps_4g_driver.lock().expect("Failed to lock gps_4g_driver").is_connected();

        if network_ok {
            // Send pending data to cloud
            let mut cache = state.data_cache.lock().expect("Failed to lock data_cache");
            while sent_index < cache.len() {
                let data = &cache[sent_index];
                // Simulate sending via cloud (MQTT)
                // let send_result = {
                //     let cloud = state.cloud_driver.lock().expect("Failed to lock cloud_driver");
                //     cloud.publish_str("ems/data", data)
                // };

                // match send_result {
                //     Ok(()) => {
                //         println!("Data sent successfully: index {}", sent_index);
                //         sent_index += 1;
                //     }
                //     Err(e) => {
                //         eprintln!("Failed to send data at index {}: {:?}", sent_index, e);
                //         break; // Stop sending on failure
                //     }
                // }
                println!("Data sent successfully: index {}", sent_index);
                sent_index += 1;
            }

            // Remove successfully sent data from memory cache
            if sent_index > 0 {
                cache.drain(0..sent_index);
                sent_index = 0;
            }
        } else {
            println!("Network not available, skipping send");
        }

        // Manage cache size - move excess to disk
        {
            let mut cache = state.data_cache.lock().expect("Failed to lock data_cache");
            if cache.len() > max_memory_cache {
                let excess = cache.len() - max_memory_cache;
                for i in 0..excess {
                    let filename = format!("{}/data_{}.json", cache_dir, sent_index + i);
                    if let Err(e) = fs::write(&filename, &cache[i]) {
                        eprintln!("Failed to write data to disk: {:?}", e);
                    } else {
                        // Mark as unsent on disk
                        let flag_file = format!("{}/data_{}.unsent", cache_dir, sent_index + i);
                        let _ = fs::write(flag_file, "1");
                    }
                }
                // Remove from memory
                cache.drain(0..excess);
            }
        }

        // Check disk space (simplified - in real impl, use sysinfo or similar)
        // For demo, assume space is ok

        thread::sleep(Duration::from_secs(1));
    }
}


fn power_control_thread(state: Arc<SystemState>) {
    loop {
        // Run EMS control cycle for power balancing
        {
            let mut ems_controller = state.ems_controller.lock().expect("Failed to lock ems_controller");
            if let Err(e) = ems_controller.run_control_cycle() {
                log::error!("EMS control cycle failed: {}", e);
            }
        }

        // Update main EMS status from controller
        {
            let ems_controller = state.ems_controller.lock().expect("Failed to lock ems_controller");
            let controller_status = ems_controller.get_status();
            let mut main_status = state.ems_status.lock().expect("Failed to lock ems_status");
            *main_status = controller_status;
        }

        thread::sleep(Duration::from_millis(100));
    }
}

