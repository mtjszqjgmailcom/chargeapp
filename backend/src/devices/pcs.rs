// PCS 功率转换系统
// Placeholder: PCS 设备抽象接口

use crate::types::*;

pub struct PcsDevice {
    // TODO: PCS 设备属性
}

impl PcsDevice {
    pub fn new() -> Self {
        Self {}
    }
    
    pub fn read_status(&self) -> PcsStatus {
        // TODO: 读取 PCS 状态
        PcsStatus::default()
    }
}
