// 后端类型定义
// Placeholder: Rust 数据结构

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EmsStatus {
    // TODO: EMS 系统状态
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PvStatus {
    // TODO: PV 状态
    // voltage: f32,
    // current: f32,
    // power: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BatteryStatus {
    // TODO: 电池状态
    // soc: f32,
    // voltage: f32,
    // current: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GensetStatus {
    // TODO: 发电机状态
    // running: bool,
    // power: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChargerStatus {
    // TODO: 充电器状态
    // charging: bool,
    // power: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PcsStatus {
    // TODO: PCS 状态
    // mode: String,
    // power: f32,
}
