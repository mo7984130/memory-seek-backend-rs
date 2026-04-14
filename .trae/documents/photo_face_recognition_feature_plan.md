# Photo模块人脸识别Feature拆分计划

## 目标
将photo模块中与人脸识别/机器学习相关的功能拆分为独立的 `face_recognition` feature，使得不需要人脸识别功能的部署可以减少依赖和编译时间。

## 当前结构分析

### 与模型相关的功能
1. **服务层**:
   - `face_service.rs` - 人脸检测、识别、聚类
   - `feature_service.rs` - 人脸特征管理（依赖vector_utils）

2. **控制器**:
   - `face_controller.rs` - 所有人脸相关的API

3. **聚类模块**:
   - `clustering/union_find.rs` - Union-Find聚类算法
   - `clustering/vector_utils.rs` - 向量计算工具

4. **依赖**:
   - `face_engine` - 人脸识别引擎
   - `ndarray` - 数值计算
   - `rayon` - 并行计算

5. **photo_service.rs中的部分**:
   - `FaceTask` 结构体
   - `upload_photo` 中发送人脸检测任务的逻辑

## 实施步骤

### 1. 修改 `domains/photo/Cargo.toml`
- 添加新feature `face_recognition`
- 将 `face_engine`、`ndarray`、`rayon` 设为可选依赖，仅在 `face_recognition` feature启用时编译
- `controller` feature 添加对 `face_recognition` 的可选依赖（用于FaceController）

### 2. 修改 `domains/photo/src/lib.rs`
- 为 `clustering` 模块添加 `#[cfg(feature = "face_recognition")]`
- 为 `FaceController` 的导出添加条件编译

### 3. 修改 `domains/photo/src/services/mod.rs`
- 为 `face_service` 和 `feature_service` 模块添加 `#[cfg(feature = "face_recognition")]`

### 4. 修改 `domains/photo/src/services/photo_service.rs`
- 为 `FaceTask` 结构体添加 `#[cfg(feature = "face_recognition")]`
- 为 `upload_photo` 中发送人脸检测任务的代码添加条件编译

### 5. 修改 `domains/photo/src/controller/mod.rs`
- 为 `face_controller` 模块添加 `#[cfg(feature = "face_recognition")]`

### 6. 修改 `domains/photo/src/state.rs`
- 为 `face_tx` 字段添加 `#[cfg(feature = "face_recognition")]`

### 7. 修改 `domains/photo/src/mappers/mod.rs`
- 检查 `face_feature_mapper` 和 `face_person_mapper` 是否需要条件编译
- 这些mapper在feature_service中使用，需要条件编译

### 8. 修改 `domains/photo/src/models/mod.rs`
- 检查 `face` 模型是否需要条件编译

### 9. 修改 `server/Cargo.toml`
- 添加 `face_recognition` feature，依赖 `photo/face_recognition`

### 10. 修改 `server/src/main.rs`
- 为 `face_engine`、`LazyFaceEngine`、`mpsc` 相关代码添加 `#[cfg(feature = "face_recognition")]`
- 为 `FaceController` 路由添加条件编译

### 11. 修改 `server/src/state.rs`
- 为 `face_tx` 字段添加 `#[cfg(feature = "face_recognition")]`

## Feature依赖关系

```
photo:
  - controller: [dep:axum, dep:validator, dep:tower-http, dep:tokio-util]
  - metrics: [common/metrics, dep:metrics]
  - face_recognition: [dep:face_engine, dep:ndarray, dep:rayon]

server:
  - auth: [dep:email, auth/controller]
  - user: [dep:user, user/controller]
  - photo: [dep:photo, photo/controller, dep:oss, dep:face_engine, dep:tokio]
  - face_recognition: [photo/face_recognition]
  - basic_metrics: [dep:metrics-exporter-prometheus, dep:metrics-process, dep:metrics-util]
  - metrics: [basic_metrics, auth/metrics, user/metrics, photo/metrics]
```

## 测试计划
1. 测试 `--features photo` (不包含face_recognition)
2. 测试 `--features photo,face_recognition`
3. 测试 `--features auth,user,photo` (默认)
4. 测试 `--features auth,user,photo,face_recognition`
5. 测试 `--features auth,user,photo,face_recognition,metrics`
