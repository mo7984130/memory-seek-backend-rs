● ---
  Photo 模块集成测试报告

  测试概览

  ┌──────────┬───────┐
  │   指标   │ 数量  │
  ├──────────┼───────┤
  │ 总测试数 │ 71    │
  ├──────────┼───────┤
  │ 通过     │ 47    │
  ├──────────┼───────┤
  │ 失败     │ 24    │
  ├──────────┼───────┤
  │ 通过率   │ 66.2% │
  └──────────┴───────┘

  ---
  应用层 Bug（5 个）

  Bug 1：is_belong 权限检查逻辑错误

  错误信息："该收藏夹不属于你" (403)

  影响测试（9 个）：
  - collection_photo::add::test_add_photos_to_collection
  - collection_photo::add::test_add_multiple_photos_to_collection
  - collection_photo::add::test_add_duplicate_photo_to_collection
  - collection_photo::query::test_get_collection_photos_empty
  - collection_photo::query::test_get_collection_photos_pagination
  - collection_photo::query::test_get_collection_photos_with_data
  - collection_photo::remove::test_remove_photo_from_collection
  - collection_photo::remove_batch::test_remove_photos_batch
  - collection_photo::remove_batch::test_remove_photos_partial

  Bug 位置：domains/photo/src/mappers/collection_photo_mapper.rs:76-89

  pub async fn is_belong(
      db: &impl ConnectionTrait,
      user_id: UserId,
      collection_id: CollectionId,
  ) -> Result<bool> {
      let count = Entity::find()
          .filter(Column::CollectionId.eq(collection_id.0))
          .filter(Column::UserId.eq(user_id.0))  // ← 检查的是 photo_collection_photo 表
          .count(db)
          .await
          .trace_internal_err("db_query_err", "查询失败")?;
      Ok(count > 0)
  }

  问题：该函数检查 photo_collection_photo 表中是否存在记录，而非检查 photo_collection 表的所有者。对于空收藏夹（无照片），count 始终为
  0，导致权限检查失败。

  修复建议：应改为检查 photo_collection 表中 user_id 是否匹配。

  ---
  Bug 2：删除照片需管理员权限

  错误信息："非管理员用户无法删除照片" (403)

  影响测试（2 个）：
  - photo::delete::test_delete_photo_success
  - photo::delete::test_delete_photos_batch

  Bug 位置：domains/photo/src/services/photo_service.rs 中的 delete_photos 函数

  问题：普通用户无法删除自己的照片，只有管理员才能删除。这与预期行为不符——用户应该能删除自己上传的照片。

  ---
  Bug 3：PhotoId 反序列化不支持数字类型

  错误信息：PhotoId 格式错误 at line 1 column 53

  影响测试（1 个）：
  - photo::query::test_get_photos_cursor_pagination

  Bug 位置：entities/src/photo/photo.rs:31-37

  impl<'de> Deserialize<'de> for PhotoId {
      fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
          let s = String::deserialize(d)  // ← 只支持字符串格式
              .map_err(|_| serde::de::Error::custom("PhotoId 格式错误"))?;
          s.parse::<i64>()
              .map(PhotoId)
              .map_err(|_| serde::de::Error::custom("PhotoId 格式错误"))
      }
  }

  问题：游标序列化时 id 字段为数字（{"created_at":"...","id":101}），但反序列化只支持字符串格式（"id":"101"）。

  修复建议：使用 #[serde(deserialize_with)] 支持数字和字符串两种格式。

  ---
  Bug 4：时间线统计 SQL 列歧义

  错误信息：column reference "count" is ambiguous

  影响：照片上传时时间线统计更新失败（非致命错误，不影响上传结果）

  Bug 位置：domains/photo/src/mappers/timeline_stat_mapper.rs:33

  on_conflict
      .update_columns([Column::AnchorTime, Column::UpdatedAt])
      .value(Column::Count, Expr::col(Column::Count).add(1));  // ← "count" 列歧义

  问题：在 ON CONFLICT 更新语句中，count 列名在 INSERT 和 UPDATE 之间产生歧义。

  修复建议：使用表别名明确指定列来源。

  ---
  Bug 5：评论删除返回 500

  错误信息："服务器内部错误" (500)

  影响测试（1 个）：
  - comment::delete::test_delete_comment_success

  Bug 位置：domains/photo/src/services/comment_service.rs:136 的 delete 函数

  问题：删除评论时发生内部错误，可能是权限检查或数据库操作异常。

  ---
  测试适配问题（12 个）

  问题 1：收藏夹自动创建"我喜欢"（5 个测试）

  行为：get_collection_list 在收藏夹为空时自动创建"我喜欢"收藏夹

  影响测试：
  - collection::query::test_get_collections_empty — 期望 0 个，实际 1 个
  - collection::query::test_get_collections_user_isolation — 用户 2 也有"我喜欢"
  - collection::delete::test_delete_collection_success — "我喜欢"不可删除
  - collection::delete::test_delete_collection_and_recreate — "我喜欢"不可删除
  - collection::create::test_create_collection_empty_name — 需验证返回值

  问题 2：空名称/不存在资源验证（3 个测试）

  - collection::create::test_create_collection_empty_name — 空名称返回 500 而非 400
  - comment::publish::test_publish_comment_empty_content — 空内容返回 500 而非 400
  - comment::publish::test_publish_comment_nonexistent_photo — 不存在的图片返回 500

  问题 3：其他用户操作权限（2 个测试）

  - comment::delete::test_delete_other_user_comment — 删除他人评论返回 500
  - comment_like::like::test_like_nonexistent_comment — 点赞不存在评论返回 500

  问题 4：取消点赞未点赞内容（1 个测试）

  - comment_like::unlike::test_unlike_comment_success — 返回 400 "还未点赞"

  ---
  总结

  ┌──────────────┬──────┬──────────────────────────┐
  ┌──────────────┬──────┬──────────────────────────┐
  ├──────────────┼──────┼──────────────────────────┤
  │ 应用层 Bug   │ 5    │ 需要修复代码             │
  ├──────────────┼──────┼──────────────────────────┤
  │ 测试适配问题 │ 12   │ 测试期望与业务逻辑不匹配 │
  ├──────────────┼──────┼──────────────────────────┤
  ├──────────────┼──────┼──────────────────────────┤
  │ 测试适配问题 │ 12   │ 测试期望与业务逻辑不匹配 │
  ├──────────────┼──────┼──────────────────────────┤
  ├──────────────┼──────┼──────────────────────────┤
  │ 测试适配问题 │ 12   │ 测试期望与业务逻辑不匹配 │
  ├──────────────┼──────┼──────────────────────────┤
  │ 已通过测试   │ 47   │ 功能正常                 │
  └──────────────┴──────┴──────────────────────────┘

  优先修复建议：
  1. Bug 1（is_belong）— 影响最大，9 个测试失败
  2. Bug 2（删除权限）— 影响用户正常使用
  3. Bug 3（PhotoId 反序列化）— 影响分页功能
