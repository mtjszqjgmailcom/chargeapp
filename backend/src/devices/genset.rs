// Genset 发电机设备
// Placeholder: Genset 设备抽象接口

use crate::types::*;

pub struct GensetDevice {
    // TODO: Genset 设备属性
}

impl GensetDevice {
    pub fn new() -> Self {
        Self {}
    }
    
    pub fn read_status(&self) -> GensetStatus {
        // TODO: 读取发电机状态
        GensetStatus::default()
    }
}
