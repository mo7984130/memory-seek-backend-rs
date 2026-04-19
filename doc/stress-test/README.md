# k6 压力测试脚本

## 目录结构

```
stress-test/
├── common/                         # 公共模块
│   ├── config.js                  # 配置文件
│   ├── auth.js                    # 认证工具函数
│   └── utils.js                   # 工具函数
├── photo/                          # Photo模块压测脚本
│   ├── photo_service.js           # 照片服务压测
│   ├── collection_service.js      # 收藏夹服务压测
│   ├── comment_service.js         # 评论服务压测
│   ├── face_service.js            # 人脸服务压测
│   ├── feature_service.js         # 人脸特征服务压测
│   ├── timeline_stat_service.js   # 时间线统计服务压测
│   └── photos.js                  # 照片浏览综合测试
├── main.js                         # 主测试套件
├── mixed.js                        # 混合场景测试
└── README.md                       # 文档
```

## 安装 k6

### Windows
```powershell
# 使用 Chocolatey
choco install k6

# 或使用 Scoop
scoop install k6

# 或下载安装包
# https://github.com/grafana/k6/releases
```

### macOS
```bash
brew install k6
```

### Linux
```bash
# Debian/Ubuntu
sudo gpg -k
sudo gpg --no-default-keyring --keyring /usr/share/keyrings/k6-archive-keyring.gpg --keyserver hkp://keyserver.ubuntu.com:80 --recv-keys C5AD17C747E3415A3642D57D77C6C491438B9D1F
echo "deb [signed-by=/usr/share/keyrings/k6-archive-keyring.gpg] https://dl.k6.io/deb stable main" | sudo tee /etc/apt/sources.list.d/k6.list
sudo apt update
sudo apt install k6
```

## 配置测试账号

编辑 `common/config.js` 文件，添加你的测试账号：

```javascript
testUsers: [
    { account: 'testuser01', password: 'Test1234' },
    { account: 'testuser02', password: 'Test1234' },
    // ... 添加更多账号
],
```

## 运行测试

### Photo模块Service压测

#### 1. Photo Service - 照片核心服务
```bash
# 照片游标分页、MD5检查、时间范围查询
k6 run photo/photo_service.js
```

**测试接口：**
- GET /photo/photo/cursor - 游标分页查询照片列表
- GET /photo/photo/md5-exist - 检查MD5是否存在
- GET /photo/photo/time-range - 获取照片时间范围

#### 2. Collection Service - 收藏夹服务
```bash
# 收藏夹CRUD操作、照片收藏管理
k6 run photo/collection_service.js
```

**测试接口：**
- GET /photo/collection - 获取收藏夹列表
- POST /photo/collection - 创建收藏夹
- PUT /photo/collection/{id} - 编辑收藏夹
- DELETE /photo/collection/{id} - 删除收藏夹
- POST /photo/collection/{id}/photo - 添加照片到收藏夹
- DELETE /photo/collection/{id}/photo - 从收藏夹移除照片
- GET /photo/collection/{id}/photos - 获取收藏夹照片列表

#### 3. Comment Service - 评论服务
```bash
# 评论发布、点赞、删除
k6 run photo/comment_service.js
```

**测试接口：**
- GET /photo/comment/photo/{photoId} - 获取照片评论列表
- POST /photo/comment - 发布评论
- DELETE /photo/comment/{id} - 删除评论
- POST /photo/comment/{id}/like - 切换点赞状态

#### 4. Face Service - 人脸识别服务
```bash
# 人物列表、搜索、详情、照片浏览
k6 run photo/face_service.js
```

**测试接口：**
- GET /photo/face/person - 获取人物列表
- GET /photo/face/person/all - 获取所有人物简单列表
- GET /photo/face/person/{id} - 获取人物详情
- PUT /photo/face/person/{id} - 重命名人物
- POST /photo/face/person/merge - 合并人物
- DELETE /photo/face/person/{id} - 删除人物
- GET /photo/face/person/{id}/photos - 获取人物照片
- GET /photo/face/person/search - 搜索人物

#### 5. Feature Service - 人脸特征服务
```bash
# 人脸特征查询、删除、归属更改
k6 run photo/feature_service.js
```

**测试接口：**
- GET /photo/feature/photo/{photoId} - 获取照片人脸特征
- DELETE /photo/feature/{id} - 删除人脸特征
- PUT /photo/feature/{id}/person - 更改人脸归属

#### 6. Timeline Stat Service - 时间线统计服务
```bash
# 时间线统计查询
k6 run photo/timeline_stat_service.js
```

**测试接口：**
- GET /photo/timeline-stat - 获取时间线统计

### 综合测试场景

```bash
# 照片浏览综合测试
k6 run photo/photos.js

# 混合业务场景测试
k6 run mixed.js

# 完整测试套件（约 22 分钟）
k6 run main.js
```

### 输出 JSON 报告

```bash
k6 run --out json=report.json photo/photo_service.js
```

## 测试场景说明

### Photo模块Service压测脚本

| 脚本 | Service | 包含操作 | 并发数 | 持续时间 |
|------|---------|---------|--------|----------|
| photo_service.js | PhotoService | 游标分页、MD5检查、时间范围 | 50 | ~5 分钟 |
| collection_service.js | CollectionService | 收藏夹CRUD、照片收藏管理 | 30 | ~4 分钟 |
| comment_service.js | CommentService | 评论发布、点赞、删除 | 20 | ~4 分钟 |
| face_service.js | FaceService | 人物列表、搜索、详情、照片 | 20 | ~4 分钟 |
| feature_service.js | FeatureService | 人脸特征查询、删除、归属更改 | 20 | ~4 分钟 |
| timeline_stat_service.js | TimelineStatService | 时间线统计查询 | 50 | ~5 分钟 |

### 完整测试套件（main.js）

| 场景 | 说明 | 并发数 | 开始时间 | 持续时间 |
|------|------|--------|---------|----------|
| 登录测试 | 纯登录场景 | 50 | 0m | ~4 分钟 |
| 照片浏览 | 瀑布流浏览 | 50 | 4m | ~6 分钟 |
| 收藏夹管理 | 完整 CRUD 操作 | 20 | 10m | ~4 分钟 |
| 评论系统 | 发布/点赞/删除 | 15 | 13m | ~4 分钟 |
| 人物浏览 | 列表/搜索/详情 | 15 | 16m | ~4 分钟 |
| 混合场景 | 完整用户旅程 | 40 | 20m | ~10 分钟 |

**总计持续时间：约 22 分钟**

## 测试覆盖的功能

### ✅ 读操作
- [x] 用户登录
- [x] Token 刷新
- [x] 照片游标分页查询
- [x] MD5存在性检查
- [x] 照片时间范围查询
- [x] 收藏夹列表
- [x] 收藏夹内照片
- [x] 人物列表分页
- [x] 人物搜索
- [x] 人物详情
- [x] 人物的照片
- [x] 评论列表
- [x] 照片人脸特征
- [x] 时间线统计

### ✅ 写操作
- [x] **创建收藏夹**
- [x] **删除收藏夹**
- [x] **添加照片到收藏夹**
- [x] **从收藏夹移除照片**
- [x] **发布评论**
- [x] **删除评论**
- [x] **点赞/取消点赞评论**
- [x] **删除人脸特征**
- [x] **更改人脸归属**

### ❌ 未包含的功能
- [ ] 照片上传（需要实际图片文件）
- [ ] 人脸检测和聚类（后台任务）
- [ ] 人物重命名（非核心场景）
- [ ] 人物合并（需要预置数据）

## 验收标准

### 读操作指标
| 指标 | 目标值 |
|------|--------|
| P95 响应时间 | < 500ms |
| P99 响应时间 | < 1000ms |
| 错误率 | < 1% |
| 吞吐量 | > 100 TPS |

### 写操作指标
| 指标 | 目标值 |
|------|--------|
| P95 响应时间 | < 800ms |
| P99 响应时间 | < 1500ms |
| 错误率 | < 2% |
| 吞吐量 | > 50 TPS |

## 测试数据管理

### 数据隔离
- 每个虚拟用户使用独立的测试数据
- 通过时间戳生成唯一的收藏夹名、评论内容
- 避免多个 VU 操作同一条数据

### 数据清理
- 测试中创建的收藏夹会在测试完成后删除
- 测试中发布的评论会在测试完成后删除
- 确保不会产生垃圾数据

## 测试报告

测试完成后会生成：
- **控制台输出摘要**：实时显示测试结果
- **stress-test-report.json**：详细的 JSON 格式报告文件

### 报告包含的指标
- HTTP 请求响应时间（平均、P95、P99）
- 请求错误率
- 总请求数和每秒请求数（RPS）
- 迭代次数
- 各场景的详细指标

## 常见问题

### Q: 测试失败怎么办？
A: 检查以下几点：
1. 测试账号是否正确配置
2. 后端服务是否正常运行
3. 网络连接是否正常
4. 查看控制台输出的详细错误信息

### Q: 如何调整并发数？
A: 编辑对应脚本的 `options.stages` 配置，修改 `target` 值

### Q: 如何只测试某个场景？
A: 使用 `k6 run photo/<脚本名>.js` 运行单个场景

### Q: 测试数据会残留吗？
A: 不会。每个场景都会在测试完成后清理自己创建的数据

## 最佳实践

1. **先在测试环境运行**：确保脚本正常工作
2. **从小并发开始**：逐步增加并发数观察系统表现
3. **监控系统资源**：同时监控 CPU、内存、数据库性能
4. **定期执行**：建议每次重大更新后执行压测
5. **保存报告**：对比不同时期的性能数据

## 技术栈

- **测试工具**: k6
- **脚本语言**: JavaScript (ES6)
- **测试类型**: 负载测试、压力测试、集成测试
