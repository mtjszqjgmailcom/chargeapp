// Modbus TCP 客户端实现
// 提供与 Modbus 设备的通信接口，支持读取和写入寄存器

use std::time::Duration;
use tokio_modbus::client::sync::{tcp::connect, tcp::Client};
use tokio_modbus::prelude::*;

/// Modbus 通信错误类型
#[derive(Debug, Clone)]
pub enum ModbusError {
    ConnectionFailed(String),
    Timeout,
    ProtocolError(String),
    InvalidData(String),
}

impl std::fmt::Display for ModbusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModbusError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            ModbusError::Timeout => write!(f, "Operation timed out"),
            ModbusError::ProtocolError(msg) => write!(f, "Protocol error: {}", msg),
            ModbusError::InvalidData(msg) => write!(f, "Invalid data: {}", msg),
        }
    }
}

impl std::error::Error for ModbusError {}

/// Modbus TCP 客户端结构体
/// 提供同步 Modbus TCP 通信功能，支持连接管理和错误处理
pub struct ModbusClient {
    /// 目标主机地址
    host: String,
    /// 目标端口
    port: u16,
    /// 连接超时时间
    timeout: Duration,
    /// Modbus 单元标识符 (通常为 1)
    unit_id: u8,
    /// TCP 客户端连接，可选以支持延迟连接
    client: Option<Client>,
}

impl ModbusClient {
    /// 创建新的 ModbusClient 实例
    ///
    /// # 参数
    /// * `host` - Modbus 服务器主机地址
    /// * `port` - Modbus 服务器端口
    ///
    /// # 返回
    /// 返回配置好的 ModbusClient 实例，默认超时 5 秒，unit_id 1
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            host: host.to_string(),
            port,
            timeout: Duration::from_secs(5),
            unit_id: 1,
            client: None,
        }
    }

    /// 使用自定义配置创建 ModbusClient
    ///
    /// # 参数
    /// * `host` - Modbus 服务器主机地址
    /// * `port` - Modbus 服务器端口
    /// * `timeout` - 操作超时时间
    /// * `unit_id` - Modbus 单元标识符
    pub fn with_config(host: &str, port: u16, timeout: Duration, unit_id: u8) -> Self {
        Self {
            host: host.to_string(),
            port,
            timeout,
            unit_id,
            client: None,
        }
    }

    /// 连接到 Modbus 服务器
    ///
    /// # 返回
    /// 成功时返回 Ok(()), 失败时返回 ModbusError
    pub fn connect(&mut self) -> Result<(), ModbusError> {
        let socket_addr = format!("{}:{}", self.host, self.port)
            .parse()
            .map_err(|e| ModbusError::ConnectionFailed(format!("Invalid address: {}", e)))?;

        match connect(socket_addr) {
            Ok(client) => {
                self.client = Some(client);
                Ok(())
            }
            Err(e) => Err(ModbusError::ConnectionFailed(e.to_string())),
        }
    }

    /// 断开与 Modbus 服务器的连接
    pub fn disconnect(&mut self) {
        self.client = None;
    }

    /// 检查连接是否已建立
    pub fn is_connected(&self) -> bool {
        self.client.is_some()
    }

    /// 读取保持寄存器 (Holding Registers)
    ///
    /// # 参数
    /// * `address` - 起始寄存器地址
    /// * `count` - 要读取的寄存器数量
    ///
    /// # 返回
    /// 成功时返回寄存器值的向量，失败时返回 ModbusError
    pub fn read_holding_registers(&mut self, address: u16, count: u16) -> Result<Vec<u16>, ModbusError> {
        let client = self.client.as_mut()
            .ok_or_else(|| ModbusError::ConnectionFailed("Not connected".to_string()))?;

        match client.read_holding_registers(address, count) {
            Ok(values) => Ok(values),
            Err(e) => Err(ModbusError::ProtocolError(e.to_string())),
        }
    }

    /// 读取输入寄存器 (Input Registers)
    ///
    /// # 参数
    /// * `address` - 起始寄存器地址
    /// * `count` - 要读取的寄存器数量
    ///
    /// # 返回
    /// 成功时返回寄存器值的向量，失败时返回 ModbusError
    pub fn read_input_registers(&mut self, address: u16, count: u16) -> Result<Vec<u16>, ModbusError> {
        let client = self.client.as_mut()
            .ok_or_else(|| ModbusError::ConnectionFailed("Not connected".to_string()))?;

        match client.read_input_registers(address, count) {
            Ok(values) => Ok(values),
            Err(e) => Err(ModbusError::ProtocolError(e.to_string())),
        }
    }

    /// 写入单个保持寄存器
    ///
    /// # 参数
    /// * `address` - 寄存器地址
    /// * `value` - 要写入的值
    ///
    /// # 返回
    /// 成功时返回 Ok(()), 失败时返回 ModbusError
    pub fn write_single_register(&mut self, address: u16, value: u16) -> Result<(), ModbusError> {
        let client = self.client.as_mut()
            .ok_or_else(|| ModbusError::ConnectionFailed("Not connected".to_string()))?;

        match client.write_single_register(address, value) {
            Ok(_) => Ok(()),
            Err(e) => Err(ModbusError::ProtocolError(e.to_string())),
        }
    }

    /// 写入多个保持寄存器
    ///
    /// # 参数
    /// * `address` - 起始寄存器地址
    /// * `values` - 要写入的值的切片
    ///
    /// # 返回
    /// 成功时返回 Ok(()), 失败时返回 ModbusError
    pub fn write_multiple_registers(&mut self, address: u16, values: &[u16]) -> Result<(), ModbusError> {
        let client = self.client.as_mut()
            .ok_or_else(|| ModbusError::ConnectionFailed("Not connected".to_string()))?;

        match client.write_multiple_registers(address, values) {
            Ok(_) => Ok(()),
            Err(e) => Err(ModbusError::ProtocolError(e.to_string())),
        }
    }

    /// 读取线圈状态 (Coils)
    ///
    /// # 参数
    /// * `address` - 起始线圈地址
    /// * `count` - 要读取的线圈数量
    ///
    /// # 返回
    /// 成功时返回线圈状态的向量，失败时返回 ModbusError
    pub fn read_coils(&mut self, address: u16, count: u16) -> Result<Vec<bool>, ModbusError> {
        let client = self.client.as_mut()
            .ok_or_else(|| ModbusError::ConnectionFailed("Not connected".to_string()))?;

        match client.read_coils(address, count) {
            Ok(states) => Ok(states),
            Err(e) => Err(ModbusError::ProtocolError(e.to_string())),
        }
    }

    /// 读取离散输入状态 (Discrete Inputs)
    ///
    /// # 参数
    /// * `address` - 起始离散输入地址
    /// * `count` - 要读取的离散输入数量
    ///
    /// # 返回
    /// 成功时返回离散输入状态的向量，失败时返回 ModbusError
    pub fn read_discrete_inputs(&mut self, address: u16, count: u16) -> Result<Vec<bool>, ModbusError> {
        let client = self.client.as_mut()
            .ok_or_else(|| ModbusError::ConnectionFailed("Not connected".to_string()))?;

        match client.read_discrete_inputs(address, count) {
            Ok(states) => Ok(states),
            Err(e) => Err(ModbusError::ProtocolError(e.to_string())),
        }
    }

    /// 写入单个线圈 (Write Single Coil)
    ///
    /// # 参数
    /// * `address` - 线圈地址
    /// * `value` - 要写入的值 (true/false)
    ///
    /// # 返回
    /// 成功时返回 Ok(()), 失败时返回 ModbusError
    pub fn write_single_coil(&mut self, address: u16, value: bool) -> Result<(), ModbusError> {
        let client = self.client.as_mut()
            .ok_or_else(|| ModbusError::ConnectionFailed("Not connected".to_string()))?;

        match client.write_single_coil(address, value) {
            Ok(_) => Ok(()),
            Err(e) => Err(ModbusError::ProtocolError(e.to_string())),
        }
    }

    /// 写入多个线圈 (Write Multiple Coils)
    ///
    /// # 参数
    /// * `address` - 起始线圈地址
    /// * `values` - 要写入的值的切片
    ///
    /// # 返回
    /// 成功时返回 Ok(()), 失败时返回 ModbusError
    pub fn write_multiple_coils(&mut self, address: u16, values: &[bool]) -> Result<(), ModbusError> {
        let client = self.client.as_mut()
            .ok_or_else(|| ModbusError::ConnectionFailed("Not connected".to_string()))?;

        match client.write_multiple_coils(address, values) {
            Ok(_) => Ok(()),
            Err(e) => Err(ModbusError::ProtocolError(e.to_string())),
        }
    }
}

impl Drop for ModbusClient {
    /// 在结构体销毁时自动断开连接
    fn drop(&mut self) {
        self.disconnect();
    }
}
