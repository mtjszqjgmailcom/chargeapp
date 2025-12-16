// CAN 设备驱动 (socketcan + can-frame)
// CAN 总线通信模块，提供可靠的 CAN 通信接口

use socketcan::CanSocket;
use socketcan::CanFrame;
use socketcan::Socket;
use std::io;
use std::fmt;
use std::time::Duration;

/// CAN 通信错误类型
#[derive(Debug, Clone)]
pub enum CanError {
    /// 连接失败
    ConnectionFailed(String),
    /// 超时错误
    Timeout,
    /// 协议错误
    ProtocolError(String),
    /// 无效数据
    InvalidData(String),
    /// 配置错误
    ConfigError(String),
}

impl std::fmt::Display for CanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CanError::ConnectionFailed(msg) => write!(f, "Connection failed: {}", msg),
            CanError::Timeout => write!(f, "Operation timed out"),
            CanError::ProtocolError(msg) => write!(f, "Protocol error: {}", msg),
            CanError::InvalidData(msg) => write!(f, "Invalid data: {}", msg),
            CanError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
        }
    }
}

impl std::error::Error for CanError {}

impl From<io::Error> for CanError {
    fn from(error: io::Error) -> Self {
        CanError::ProtocolError(error.to_string())
    }
}

/// CAN 配置结构体
/// 定义 CAN 接口的配置参数
#[derive(Debug, Clone)]
pub struct CanConfig {
    /// CAN 接口名称（如 "can0"）
    pub interface: String,
    /// 波特率 (bit/s)，如 500000
    pub bitrate: u32,
    /// 采样点位置 (0.0-1.0)，默认 0.875
    pub sample_point: f32,
    /// 是否启用回环模式（用于测试）
    pub loopback: bool,
    /// 是否只监听模式
    pub listen_only: bool,
    /// 总线重启延迟（毫秒）
    pub restart_ms: u32,
    /// 接收超时时间
    pub timeout: Duration,
}

impl Default for CanConfig {
    fn default() -> Self {
        Self {
            interface: "can0".to_string(),
            bitrate: 500_000,
            sample_point: 0.875,
            loopback: false,
            listen_only: false,
            restart_ms: 100,
            timeout: Duration::from_secs(5),
        }
    }
}

impl CanConfig {
    /// 创建新的 CAN 配置
    pub fn new(interface: &str, bitrate: u32) -> Self {
        Self {
            interface: interface.to_string(),
            bitrate,
            ..Default::default()
        }
    }

    /// 验证配置参数
    pub fn validate(&self) -> Result<(), CanError> {
        if self.interface.is_empty() {
            return Err(CanError::ConfigError("Interface cannot be empty".to_string()));
        }
        if self.bitrate == 0 {
            return Err(CanError::ConfigError("Bitrate must be greater than 0".to_string()));
        }
        if !(0.0..=1.0).contains(&self.sample_point) {
            return Err(CanError::ConfigError("Sample point must be between 0.0 and 1.0".to_string()));
        }
        Ok(())
    }
}

/// CAN 设备驱动结构体
/// 提供 CAN 总线通信功能，支持连接管理和错误处理
pub struct CanDriver {
    /// CAN 配置
    config: CanConfig,
    /// CAN 套接字，可选以支持延迟连接
    socket: Option<CanSocket>,
}

impl CanDriver {
    /// 创建新的 CanDriver 实例，使用默认配置
    ///
    /// # 参数
    /// * `interface` - CAN 接口名称
    ///
    /// # 返回
    /// 返回配置好的 CanDriver 实例
    pub fn new(interface: &str) -> Self {
        Self::with_config(CanConfig::new(interface, 500_000))
    }

    /// 使用自定义配置创建 CanDriver
    ///
    /// # 参数
    /// * `config` - CAN 配置结构体
    ///
    /// # 返回
    /// 返回配置好的 CanDriver 实例
    pub fn with_config(config: CanConfig) -> Self {
        Self {
            config,
            socket: None,
        }
    }

    /// 初始化 CAN 连接
    ///
    /// # 返回
    /// 成功时返回 Ok(()), 失败时返回 CanError
    pub fn connect(&mut self) -> Result<(), CanError> {
        self.config.validate()?;

        // 注意：实际的 socketcan 设置需要通过命令行工具如 ip link set can0 type can bitrate 500000
        // 这里仅创建套接字连接
        let socket = CanSocket::open(&self.config.interface)?;
        self.socket = Some(socket);
        Ok(())
    }

    /// 断开 CAN 连接
    pub fn disconnect(&mut self) {
        self.socket = None;
    }

    /// 检查连接是否已建立
    pub fn is_connected(&self) -> bool {
        self.socket.is_some()
    }

    /// 获取当前配置
    pub fn config(&self) -> &CanConfig {
        &self.config
    }

    /// 发送 CAN 帧
    ///
    /// # 参数
    /// * `frame` - 要发送的 CAN 帧
    ///
    /// # 返回
    /// 成功时返回 Ok(()), 失败时返回 CanError
    pub fn send_frame(&self, frame: &CanFrame) -> Result<(), CanError> {
        let socket = self.socket.as_ref()
            .ok_or_else(|| CanError::ConnectionFailed("Not connected".to_string()))?;
        socket.write_frame_insist(frame)?;
        Ok(())
    }

    /// 接收 CAN 帧
    ///
    /// # 返回
    /// 成功时返回 CAN 帧，失败时返回 CanError
    pub fn recv_frame(&self) -> Result<CanFrame, CanError> {
        let socket = self.socket.as_ref()
            .ok_or_else(|| CanError::ConnectionFailed("Not connected".to_string()))?;
        socket.read_frame_timeout(self.config.timeout).map_err(Into::into)
    }

    /// 非阻塞接收 CAN 帧
    ///
    /// # 返回
    /// 成功时返回 Some(CAN 帧)，无数据时返回 None，失败时返回 CanError
    pub fn try_recv_frame(&self) -> Result<Option<CanFrame>, CanError> {
        let socket = self.socket.as_ref()
            .ok_or_else(|| CanError::ConnectionFailed("Not connected".to_string()))?;

        match socket.read_frame_timeout(Duration::from_millis(0)) {
            Ok(frame) => Ok(Some(frame)),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
}

impl Drop for CanDriver {
    /// 在结构体销毁时自动断开连接
    fn drop(&mut self) {
        self.disconnect();
    }
}

impl fmt::Debug for CanDriver {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CanDriver")
            .field("config", &self.config)
            .field("connected", &self.socket.is_some())
            .finish()
    }
}
