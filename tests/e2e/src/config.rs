//! e2e 配置加载, 读取逻辑与 server 一致:
//! 1. CLI 参数 `--config` / `-c`
//! 2. 环境变量 `E2E_CONFIG_PATH`
//! 3. 默认值 `e2e.config.yml`(运行目录)或 `tests/e2e.config.yml`
//!
//! 所有配置项均无默认值, 必须显式提供; 环境变量(前缀 `E2E`, 分隔符 `__`)可覆盖文件配置,
//! 如 `E2E__SEED__AUTH_USERS=100`。

use std::path::PathBuf;
use std::time::Duration;

use config::{Config, Environment, File};
use memseek_test::{BackoffConfig, RunMode};
use serde::Deserialize;
use tracing::info;

#[derive(Debug, Deserialize)]
pub struct E2eConfig {
    pub server: ServerConfig,

    pub database: DatabaseConfig,

    pub redis: RedisConfig,

    pub mailhog: MailhogConfig,

    /// 对象存储(头像回查), 需与 server 运行时 s3 段一致
    pub s3: S3Config,

    /// 图片 token 加密配置, 必须与 server 的 token_cipher 一致
    pub token_cipher: TokenCipherConfig,

    pub seed: SeedConfig,

    /// 运行参数(执行模式 / 并发 / 场景集 / 判定), 缺省时用内置默认值
    #[serde(default)]
    pub run: RunConfig,
}

/// e2e 运行参数。
///
/// 环境变量前缀 `E2E__RUN__` 可覆盖任意字段(如 `E2E__RUN__CONCURRENCY=32`),
/// 因此 CI 无需为不同用途生成不同的配置文件。
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct RunConfig {
    /// 执行模式: `times` = 固定轮次(正确性验收); `duration` = 固定时长(压测)
    pub mode: RunModeKind,

    /// `mode: times` 时的总轮次(所有并发任务之和)
    pub times: u64,

    /// `mode: duration` 时的时长(秒)
    pub duration_secs: u64,

    /// 并发度。必须 <= 种子池大小(auth_users / visual_users / uit_users),
    /// 场景按 `task.index` 取账号, 池子不够会串号
    pub concurrency: u64,

    /// 场景白名单(逗号分隔, 空 = 全部场景)。用于压测裁剪或单场景调试
    pub scenarios: String,

    /// 超时退避初始值(毫秒), 0 = 关闭。压测建议关闭, 否则过载会自动减速
    pub backoff_ms: u64,

    /// 是否执行前置准备(preprea: 清空种子并重新灌入)。
    /// 外部被测目标、或连续多轮压测复用同一份种子时置 false
    pub prepare: bool,

    /// 允许的失败率上限(失败 = run 失败 + validate 未通过), 超过则退出码 1
    pub max_failure_rate: f64,

    /// 允许的超时率上限(timeouts / times), 超过则退出码 1
    pub max_timeout_rate: f64,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            mode: RunModeKind::Times,
            times: 128,
            duration_secs: 120,
            concurrency: 32,
            scenarios: String::new(),
            backoff_ms: 0,
            prepare: true,
            max_failure_rate: 0.0,
            max_timeout_rate: 0.0,
        }
    }
}

/// 场景执行模式(与 `memseek_test::RunMode` 一一对应)。
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RunModeKind {
    /// 固定轮次: 正确性验收
    Times,
    /// 固定时长: 压测
    Duration,
}

impl RunConfig {
    /// 构造框架执行模式。
    pub fn run_mode(&self) -> RunMode {
        match self.mode {
            RunModeKind::Times => RunMode::Times(self.times),
            RunModeKind::Duration => RunMode::Duration(Duration::from_secs(self.duration_secs)),
        }
    }

    /// 场景白名单(空 = 全部场景)。
    pub fn scenario_whitelist(&self) -> Vec<&str> {
        self.scenarios
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// 超时退避配置(未启用时为 `None`, 超时后立即进入下一轮)。
    pub fn backoff(&self) -> Option<BackoffConfig> {
        (self.backoff_ms > 0).then(|| {
            BackoffConfig::new(
                Duration::from_millis(self.backoff_ms),
                Duration::from_secs(3),
                2.0,
            )
        })
    }

    /// 参数自检(启动即失败, 避免跑出一堆无意义结果)。
    pub fn validate(&self) -> Result<(), String> {
        if self.concurrency == 0 {
            return Err("run.concurrency 不能为 0".to_string());
        }
        match self.mode {
            RunModeKind::Times if self.times == 0 => {
                return Err("run.mode=times 时 run.times 不能为 0".to_string());
            }
            RunModeKind::Duration if self.duration_secs == 0 => {
                return Err("run.mode=duration 时 run.duration_secs 不能为 0".to_string());
            }
            _ => {}
        }
        for (name, rate) in [
            ("max_failure_rate", self.max_failure_rate),
            ("max_timeout_rate", self.max_timeout_rate),
        ] {
            if !(0.0..=1.0).contains(&rate) {
                return Err(format!("run.{name} 必须落在 [0, 1]"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    /// 被测 server 的基础地址, 可含路径前缀(如 `https://example.com/api`)
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct RedisConfig {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct MailhogConfig {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct S3Config {
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    pub region: String,
    pub bucket: String,
    pub public_url: Option<String>,
    pub force_path_style: bool,
}

impl S3Config {
    /// 转换为 `oss` 库的客户端配置.
    pub fn to_oss(&self) -> oss::S3Config {
        oss::S3Config {
            endpoint: self.endpoint.clone(),
            access_key: self.access_key.clone(),
            secret_key: self.secret_key.clone(),
            region: self.region.clone(),
            bucket: self.bucket.clone(),
            public_url: self.public_url.clone(),
            force_path_style: self.force_path_style,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct TokenCipherConfig {
    pub key: String,
    pub salt: String,
}

#[derive(Debug, Deserialize)]
pub struct SeedConfig {
    pub auth_users: u64,
    pub visual_users: u64,
    pub visuals_per_user: u64,
    pub faces_per_person: u64,
    /// user 模块专属测试用户池大小(uit_user_* / uit_pwd_*), 需 >= Manager 并发度
    pub uit_users: u64,
}

impl SeedConfig {
    pub fn visual_count(&self) -> u64 {
        self.visual_users * self.visuals_per_user
    }
}

impl E2eConfig {
    /// 加载配置, 按优先级确定配置文件路径:
    /// 1. CLI 参数 `--config` / `-c`
    /// 2. 环境变量 `E2E_CONFIG_PATH`
    /// 3. 默认值 `e2e.config.yml`(运行目录)或 `tests/e2e.config.yml`
    pub fn load(cli_config_path: Option<String>) -> Self {
        info!("加载配置文件");

        let config_path = if let Some(path) = cli_config_path {
            PathBuf::from(path)
        } else if let Ok(path) = std::env::var("E2E_CONFIG_PATH") {
            PathBuf::from(path)
        } else {
            // 依次探测: 运行目录 > 仓库根 tests/e2e/ 下的默认配置
            [
                PathBuf::from("e2e.config.yml"),
                PathBuf::from("tests/e2e.config.yml"),
                PathBuf::from("tests/e2e/e2e.config.yml"),
            ]
            .into_iter()
            .find(|path| path.exists())
            .unwrap_or_else(|| PathBuf::from("tests/e2e/e2e.config.yml"))
        };

        info!("配置文件路径: {:?}", config_path);

        let cfg = Config::builder()
            .add_source(File::from(config_path))
            .add_source(Environment::with_prefix("E2E").separator("__"))
            .build()
            .expect("构建配置失败");

        cfg.try_deserialize().expect("反序列化配置失败")
    }

    /// 返回被测 server 的访问地址(去掉末尾 `/`, 便于拼接请求路径).
    pub fn base_url(&self) -> String {
        self.server.url.trim_end_matches('/').to_string()
    }
}
