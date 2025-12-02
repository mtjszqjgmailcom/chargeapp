// 设备模块 - 包含所有设备抽象
// Placeholder: 设备模块入口

pub mod pv_dcdc;
pub mod bms;
pub mod genset;
pub mod charger;
pub mod pcs;

// 重新导出主要类型，方便外部使用
pub use pv_dcdc::PvDevice;
pub use bms::BatteryDevice;
pub use genset::GensetDevice;
pub use charger::ChargerDevice;
pub use pcs::PcsDevice;
