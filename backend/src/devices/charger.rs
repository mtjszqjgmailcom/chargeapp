// Charger 充电器设备
// Placeholder: Charger 设备抽象接口

use crate::types::*;

pub struct ChargerDevice {
    // TODO: Charger 设备属性
}

impl ChargerDevice {
    pub fn new() -> Self {
        Self {}
    }
    
    pub fn read_status(&self) -> ChargerStatus {
        // TODO: 读取充电器状态
        ChargerStatus::default()
    }
}
