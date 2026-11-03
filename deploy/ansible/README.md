# 部署 (Ansible)

生产环境为单机 systemd 部署: PostgreSQL + pgvector、Redis、Nginx(反代 + 静态前端)、后端服务。
本地/CI 的容器编排见 `tests/run/docker-compose.yml`, 两者互不影响。

## 产物分层

| 产物 | 内容 | 体积 | 发布方式 |
| --- | --- | --- | --- |
| `memory-seek-server-v<ver>-linux-x86_64.tar.gz` | 二进制 + `manifest.yaml` | ≈13 MB | 每次发版(`release.yml`) |
| `runtime-assets-<hash>.tar.gz` | `libs/`(onnxruntime) + `models/` | ≈180 MB | 内容变化时(`runtime-assets.yml`), 按内容 hash 命名 |

`manifest.yaml` 声明二进制依赖的运行时资产 hash, 部署脚本据此安装。资产内容不变时**不会重复下载**;
资产按 hash 分目录存放, 从不原地覆盖, 因此升级 libs 也不影响正在处理的请求。

人用入口: 版本 release 的说明里会写明本次依赖的运行时资产包地址; `runtime-assets` release
同时挂着**当前版本**的二进制包(旧版本自动清理), 一个页面即可拿全。部署脚本仍按版本号取包,
不依赖这些"便利链接"。

目标机布局:

```
/opt/memory-seek/
├── runtime/<hash>/{libs,models}          # 共享, 可多版本共存
├── releases/v<ver>/                      # 二进制 + libs/models 软链
├── current  -> releases/v<ver>
└── previous -> releases/v<ver-1>
```

## 首次准备

> 部署前确认两件事:
> 1. 目标版本已发布(`vX.Y.Z` release 里有 `memory-seek-server-vX.Y.Z-linux-x86_64.tar.gz`);
> 2. 该版本 `manifest.yaml` 声明的运行时资产已发布(运行一次 `Runtime Assets` workflow,
>    或在 `thirdparty/**` 变更后自动触发)。

```bash
cd deploy/ansible
ansible-galaxy collection install -r requirements.yml
# 主机清单不进仓库: 从模板复制后填一个 IP 即可(单机部署)
cp inventory.example.ini inventory/production.ini
# 密钥(已有 vault 请直接编辑补齐下面缺少的项)
ansible-vault edit group_vars/all/vault.yml
```

`group_vars/all/vault.yml` 需要包含(加密存储):

```
vault_db_password
vault_redis_password
vault_smtp_password
vault_s3_access_key
vault_s3_secret_key
vault_token_cipher_key
vault_token_cipher_salt
```

非敏感参数在 `group_vars/all/main.yml`(域名、数据库名、S3 endpoint/public_url、SMTP 地址等)。

> 媒体统一由后端 `/visual/{token}` 转发, `s3_public_url` 当前不参与下发;
> 但请保持为公网域名, 以免将来改为直接下发 OSS 链接时指向内网 endpoint。

部署版本固定在 `group_vars/all/versions.yml`(不读本地仓库状态), 发版时必须同步更新,
CI 会校验它与 `server/Cargo.toml` 一致。

## 日常部署

```bash
cd deploy/ansible

# 全栈(数据库 + Redis + Nginx + 后端; 前端与证书分别单独发布)
ansible-playbook site.yml --ask-vault-pass

# 只发后端(最常用, 版本取 versions.yml 中固定的 backend_version)
ansible-playbook server.yml --ask-vault-pass

# 回滚(版本目录与资产都还在, 秒级)
ansible-playbook server.yml -e binary_version=v2.5.0 --ask-vault-pass

# 只发前端(必须显式指定版本)
ansible-playbook frontend.yml -e frontend_version=v1.2.3 --ask-vault-pass

# 证书单独签发(域名/DNS 就绪后执行; 测试机/内网不用跑)
ansible-playbook certbot.yml --ask-vault-pass
```

部署流程是原子的: 新版本解压到独立目录 → 切换 `current` 软链 → 重启 → 请求
`/health`; 失败会自动切回 `previous` 并报错。最近 `keep_releases`(默认 3)个版本保留。

## 约定

- 部署版本固定写在 `group_vars/all/versions.yml`, 不随本地仓库变化; 发版时与 `server/Cargo.toml` 一起改, CI 会校验两者一致。
- 前端发布禁止使用 `latest`, 否则同一份 playbook 不可复现。
- vault 密码不要进仓库(`.vault_pass*` 已 gitignore), CI 里用 secret。
- 主机清单不入库: `inventory/` 整个目录已 gitignore, 只提交 `inventory.example.ini` 模板。
- 单机部署: 清单里只有一台主机(隐式 `all` 组), 所有 playbook 用 `hosts: all`,
  SSH 用户由 `ansible.cfg` 的 `remote_user` 提供(默认 root, 可用 `-u` 覆盖)。
  只跑部分角色用 `--tags db|redis|nginx|memory-seek-server`, 不需要为角色建分组。
- 证书签发独立成 `certbot.yml`, 不进 `site.yml`: 它绑定的是域名与 DNS, 与机器数量无关。
  测试机/内网不执行即可(站点保持 HTTP); 多机时把该 playbook 的 `hosts` 指向 TLS 入口组。
  将来若拆成多机, 再把角色分组加回 inventory 并改回 `hosts: <组名>`。
