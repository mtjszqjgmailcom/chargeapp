// PV DCDC 光伏设备
// Placeholder: PV 设备抽象接口

use crate::types::*;

pub struct PvDevice {
    // TODO: PV 设备属性
}

impl PvDevice {
    pub fn new() -> Self {
        Self {}
    }
    
    pub fn read_status(&self) -> PvStatus {
        // TODO: 读取 PV 状态
        PvStatus::default()
    }
}
