use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct BenchConfig {
    pub server: ServerConfig,
    pub auth: AuthConfig,
    #[serde(default)]
    pub users: Vec<UserCredential>,
    #[serde(default)]
    pub user_patterns: Vec<UserPattern>,
    pub bench: RunConfig,
    pub scenarios: Vec<ScenarioWeight>,
}

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub base_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    pub refresh_before_expiry_secs: i64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct UserCredential {
    pub account: String,
    pub password: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UserPattern {
    pub pattern: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct RunConfig {
    #[serde(default)]
    pub mode: RunMode,
    #[serde(default)]
    pub concurrency: usize,
    pub duration_secs: u64,
    pub warmup_secs: u64,
    #[serde(default)]
    pub staircase: Option<StaircaseConfig>,
}

#[derive(Debug, Deserialize)]
pub struct ScenarioWeight {
    pub name: String,
    pub weight: u32,
    #[serde(default)]
    pub needs_credentials: bool,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RunMode {
    Fixed,
    Staircase,
}

impl Default for RunMode {
    fn default() -> Self {
        Self::Fixed
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct StageConfig {
    pub concurrency: usize,
}

#[derive(Debug, Deserialize, Clone)]
pub struct StaircaseConfig {
    #[serde(default = "default_stage_duration")]
    pub stage_duration_secs: u64,
    #[serde(default = "default_cooldown")]
    pub cooldown_secs: u64,
    pub stages: Vec<StageConfig>,
}

fn default_stage_duration() -> u64 {
    30
}

fn default_cooldown() -> u64 {
    3
}

impl BenchConfig {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let mut config: BenchConfig = toml::from_str(&content)?;
        config.expand_patterns()?;
        Ok(config)
    }

    fn expand_patterns(&mut self) -> anyhow::Result<()> {
        for pat in &self.user_patterns {
            let accounts = parse_pattern(&pat.pattern)?;
            for account in accounts {
                self.users.push(UserCredential {
                    account,
                    password: pat.password.clone(),
                });
            }
        }
        Ok(())
    }

    /// Distribute users proportionally to scenario weights.
    /// Only scenarios with `needs_credentials = true` participate.
    pub fn allocate_users(&self) -> HashMap<String, Vec<UserCredential>> {
        let participating: Vec<&ScenarioWeight> = self
            .scenarios
            .iter()
            .filter(|s| s.needs_credentials && s.enabled)
            .collect();

        let total_weight: u32 = participating.iter().map(|s| s.weight).sum();
        if total_weight == 0 || self.users.is_empty() {
            return HashMap::new();
        }

        let total_users = self.users.len();
        let mut counts: Vec<usize> = participating
            .iter()
            .map(|s| s.weight as usize * total_users / total_weight as usize)
            .collect();

        // Distribute remainder to earlier scenarios
        let allocated: usize = counts.iter().sum();
        let remainder = total_users - allocated;
        for i in 0..remainder {
            counts[i] += 1;
        }

        let mut result = HashMap::new();
        let mut offset = 0;
        for (sw, count) in participating.iter().zip(counts) {
            result.insert(sw.name.clone(), self.users[offset..offset + count].to_vec());
            offset += count;
        }

        result
    }
}

/// Parse `prefix{start..end}` into `[prefix_start, prefix_start+1, ..., prefix_end-1]`
fn parse_pattern(pattern: &str) -> anyhow::Result<Vec<String>> {
    let open = pattern.find('{');
    let close = pattern.find('}');

    match (open, close) {
        (Some(o), Some(c)) if c > o => {
            let prefix = &pattern[..o];
            let range_str = &pattern[o + 1..c];
            let parts: Vec<&str> = range_str.split("..").collect();
            if parts.len() != 2 {
                anyhow::bail!("Invalid range format: {}", range_str);
            }
            let start: u64 = parts[0].parse()?;
            let end: u64 = parts[1].parse()?;
            if start >= end {
                anyhow::bail!("Invalid range: start >= end");
            }
            Ok((start..end).map(|i| format!("{}{}", prefix, i)).collect())
        }
        _ => anyhow::bail!("Pattern must contain {{start..end}}: {}", pattern),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pattern() {
        let result = parse_pattern("testuser{0..5}").unwrap();
        assert_eq!(
            result,
            vec![
                "testuser0",
                "testuser1",
                "testuser2",
                "testuser3",
                "testuser4"
            ]
        );
    }

    #[test]
    fn test_allocate_users() {
        let config = BenchConfig {
            server: ServerConfig {
                base_url: "http://localhost".into(),
            },
            auth: AuthConfig {
                refresh_before_expiry_secs: 60,
            },
            users: (0..100)
                .map(|i| UserCredential {
                    account: format!("user{}", i),
                    password: "pass".into(),
                })
                .collect(),
            user_patterns: vec![],
            bench: RunConfig {
                mode: RunMode::Fixed,
                concurrency: 10,
                duration_secs: 60,
                warmup_secs: 0,
                staircase: None,
            },
            scenarios: vec![
                ScenarioWeight {
                    name: "login".into(),
                    weight: 60,
                    needs_credentials: true,
                    enabled: true,
                },
                ScenarioWeight {
                    name: "refresh".into(),
                    weight: 30,
                    needs_credentials: true,
                    enabled: true,
                },
                ScenarioWeight {
                    name: "register".into(),
                    weight: 10,
                    needs_credentials: false,
                    enabled: true,
                },
            ],
        };

        let alloc = config.allocate_users();
        assert_eq!(alloc.len(), 2); // register not included
        assert_eq!(alloc["login"].len(), 67);
        assert_eq!(alloc["refresh"].len(), 33);
        assert!(!alloc.contains_key("register"));
    }
}
