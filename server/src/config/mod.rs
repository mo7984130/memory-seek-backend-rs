use std::net::IpAddr;
use std::path::PathBuf;

use config::{Config, Environment, File};

use serde::Deserialize;
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub server: ServerConfig,

    pub database: crate::setup::bases::database::Config,

    #[serde(default)]
    pub redis: crate::setup::bases::redis::Config,

    #[cfg(feature = "_cache")]
    #[serde(default)]
    pub cache: crate::setup::bases::cache::Config,

    #[cfg(feature = "_email")]
    pub smtp: crate::setup::libs::email::Config,

    #[cfg(feature = "_s3")]
    pub s3: crate::setup::libs::s3::Config,

    #[cfg(feature = "_token_cipher")]
    pub token_cipher: crate::setup::libs::token_cipher::Config,

    #[cfg(feature = "metrics")]
    #[serde(default)]
    pub metrics: crate::setup::bases::metrics::Config,

    #[cfg(feature = "backup")]
    #[serde(default)]
    pub backup: crate::setup::domains::backup::Config,

    #[cfg(feature = "face-engine")]
    pub face_engine: crate::setup::libs::face_engine::Config,

    #[cfg(feature = "visual")]
    #[serde(default)]
    pub visual: crate::setup::domains::visual::Config,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    /// 上传请求体上限(字节), 须大于业务校验上限(图片 20MB / 视频 512MB)
    #[serde(default = "default_max_upload_bytes")]
    pub max_upload_bytes: u64,
    /// 统一临时文件目录(上传落盘等), 服务启动时创建, 优雅关闭时删除
    #[serde(default = "default_tmp_path")]
    pub tmp_path: PathBuf,
}

impl Default for ServerConfig {
    /// 返回服务器配置的默认值
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            max_upload_bytes: default_max_upload_bytes(),
            tmp_path: default_tmp_path(),
        }
    }
}
/// 返回默认监听地址
fn default_host() -> String {
    "127.0.0.1".to_string()
}
/// 返回服务默认监听端口
const fn default_port() -> u16 {
    7984
}
/// 默认上传上限: 512MB(与业务视频上限一致)
const fn default_max_upload_bytes() -> u64 {
    512 * 1024 * 1024
}
/// 默认统一临时文件目录
fn default_tmp_path() -> PathBuf {
    PathBuf::from("./tmp")
}

impl AppConfig {
    /// 加载配置：依次尝试 CLI `--config`、环境变量 `MEMORY_SEEK_CONFIG_PATH`、默认路径
    pub fn load(cli_config_path: Option<String>) -> Self {
        info!("加载配置文件");

        let config_path = if let Some(path) = cli_config_path {
            PathBuf::from(path)
        } else if let Ok(path) = std::env::var("MEMORY_SEEK_CONFIG_PATH") {
            PathBuf::from(path)
        } else {
            let local = PathBuf::from("config.yml");

            if local.exists() {
                local
            } else {
                PathBuf::from("/etc/memory-seek-server/config.yml")
            }
        };

        info!("配置文件路径: {:?}", config_path);

        let cfg = Config::builder()
            .add_source(File::from(config_path))
            .add_source(
                // prefix_separator 需显式指定为 "_", 否则默认跟随 separator("__"),
                // 导致 MEMORY_SEEK_<SECTION>__<KEY> 形式的变量被静默跳过
                Environment::with_prefix("MEMORY_SEEK")
                    .prefix_separator("_")
                    .separator("__"),
            )
            .build()
            .expect("构建配置失败");

        cfg.try_deserialize().expect("反序列化配置失败")
    }

    /// 返回服务器绑定地址和端口
    pub fn server_addr(&self) -> String {
        format_addr(&self.server.host, self.server.port)
    }
}

/// 拼接绑定地址。
///
/// IPv6 主机无论是否带方括号(如 `::` / `[::]`)都规范化为 `[host]:port`,
/// 避免裸 `::` 被拼成非法地址 `:::port` 而在 `TcpListener::bind` 时失败;
/// 主机名(如 `localhost`)原样保留,交由 `ToSocketAddrs` 解析。
fn format_addr(host: &str, port: u16) -> String {
    let host = host.trim_matches(|c: char| c == '[' || c == ']');
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V6(ip)) => format!("[{ip}]:{port}"),
        Ok(IpAddr::V4(ip)) => format!("{ip}:{port}"),
        Err(_) => format!("{host}:{port}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;

    #[test]
    fn formats_ipv4_host() {
        assert_eq!(format_addr("0.0.0.0", 3000), "0.0.0.0:3000");
        assert_eq!(format_addr("127.0.0.1", 7984), "127.0.0.1:7984");
    }

    #[test]
    fn normalizes_ipv6_host_with_or_without_brackets() {
        assert_eq!(format_addr("::", 7984), "[::]:7984");
        assert_eq!(format_addr("[::]", 7984), "[::]:7984");
        assert_eq!(format_addr("::1", 7984), "[::1]:7984");
        assert_eq!(format_addr("2001:db8::1", 80), "[2001:db8::1]:80");
    }

    #[test]
    fn ipv6_result_parses_as_socket_addr() {
        for host in ["::", "[::]", "::1", "2001:db8::1"] {
            let addr = format_addr(host, 7984);
            addr.parse::<SocketAddr>()
                .unwrap_or_else(|e| panic!("{addr} 应可解析为 SocketAddr: {e}"));
        }
    }

    #[test]
    fn keeps_hostname_as_is() {
        assert_eq!(format_addr("localhost", 7984), "localhost:7984");
    }
}
