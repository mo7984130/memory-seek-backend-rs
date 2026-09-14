// 业务操作清单 —— dashboard 的唯一事实来源
// 加一个操作 = 在对应模块的 ops 数组追加一条;改指标名 = 只改这一处。
//
// 字段说明:
//   crate    指标前缀 `{crate}` 段(模块名)
//   rowTitle row 标题,格式 `<中文名> (<英文标识>)`
//   name     面板标题前缀(如 "登录耗时" 中的 "登录")
//   func     指标中的 {func} 段
//   steps    子步骤列表:
//              metric —— `{crate}:{func}:{step}` 的剩余段
//                写法一(histogram 不带后缀):'db_query'
//                写法二(histogram 带显式后缀):'validate_image:duration_seconds'
//              label —— 图例中文名
{
  auth: {
    crate: 'auth',
    ops: [
      {
        rowTitle: '用户登录 (login)',
        name: '登录',
        func: 'login',
        steps: [
          { metric: 'db_query', label: '数据库查询' },
          { metric: 'verify_password', label: '密码验证' },
          { metric: 'redis_set', label: 'Redis 写入' },
          { metric: 'acquire_permit', label: '获取许可' },
        ],
      },
      {
        rowTitle: '用户注册 (register)',
        name: '注册',
        func: 'register',
        steps: [
          { metric: 'verify_email_code', label: '验证邮箱验证码' },
          { metric: 'verify_inviter_code', label: '验证邀请码' },
          { metric: 'hash_password', label: '密码哈希' },
          { metric: 'db_insert', label: '数据库插入' },
        ],
      },
      {
        rowTitle: '发送邮箱验证码 (send_email_code)',
        name: '发送验证码',
        func: 'send_email_code',
        steps: [
          { metric: 'redis_set', label: 'Redis 写入' },
          { metric: 'send_message', label: '发送邮件' },
        ],
      },
      {
        rowTitle: '刷新 Token (refresh_access_token)',
        name: '刷新 Token',
        func: 'refresh_access_token',
        steps: [
          { metric: 'verify_token', label: '验证 Token' },
          { metric: 'set_token', label: '设置 Token' },
        ],
      },
    ],
  },

  user: {
    crate: 'user',
    ops: [
      {
        rowTitle: '获取用户信息 (get_user_info)',
        name: '获取用户信息',
        func: 'get_user_info',
        steps: [
          { metric: 'cache_get_or_load', label: '缓存读取' },
        ],
      },
      {
        rowTitle: '生成邀请码 (generate_inviter_code)',
        name: '生成邀请码',
        func: 'generate_inviter_code',
        steps: [
          { metric: 'redis_set', label: 'Redis 写入' },
        ],
      },
      {
        rowTitle: '修改昵称 (change_nickname)',
        name: '修改昵称',
        func: 'change_nickname',
        steps: [
          { metric: 'db_update', label: '数据库更新' },
          { metric: 'db_transaction', label: '数据库事务' },
          { metric: 'cache_invalidate', label: '缓存失效' },
          { metric: 'cache_invalidate_single', label: '缓存失效 (单条)' },
        ],
      },
      {
        rowTitle: '上传头像 (update_avatar)',
        name: '上传头像',
        func: 'update_avatar',
        steps: [
          // 写法二示例:显式 :duration_seconds 后缀的 histogram
          { metric: 'validate_image:duration_seconds', label: '图片校验' },
          { metric: 's3_upload', label: 'S3 上传' },
          { metric: 'db_transaction', label: '数据库事务' },
          { metric: 'cache_invalidate', label: '缓存失效' },
          { metric: 'cache_invalidate_single', label: '缓存失效 (单条)' },
          { metric: 's3_delete', label: 'S3 删除旧文件' },
        ],
      },
      {
        rowTitle: '修改密码 (change_password)',
        name: '修改密码',
        func: 'change_password',
        steps: [
          { metric: 'db_query', label: '数据库查询' },
          { metric: 'acquire_permit', label: '获取信号量' },
          { metric: 'verify_password', label: '密码验证' },
          { metric: 'hash_password', label: '密码哈希' },
          { metric: 'db_transaction', label: '数据库事务' },
          // 登出清理(内部复用 do_logout)
          { metric: 'redis_delete', label: 'Redis 删除' },
          { metric: 'cache_invalidate', label: '缓存失效' },
          { metric: 'cache_invalidate_single', label: '缓存失效 (单条)' },
        ],
      },
      {
        rowTitle: '登出 (logout)',
        name: '登出',
        func: 'logout',
        steps: [
          { metric: 'db_transaction', label: '数据库事务' },
          { metric: 'redis_delete', label: 'Redis 删除' },
          { metric: 'cache_invalidate', label: '缓存失效' },
          { metric: 'cache_invalidate_single', label: '缓存失效 (单条)' },
        ],
      },
      {
        rowTitle: '批量获取用户信息 (get_user_info_batch)',
        name: '批量获取用户信息',
        func: 'get_user_info_batch',
        steps: [
          { metric: 'cache_get_or_load_batch', label: '缓存批量读取' },
        ],
      },
    ],
  },

  photo: {
    crate: 'photo',
    ops: [
      {
        rowTitle: '查询照片列表 (get_photo_cursor_page)',
        name: '查询照片列表',
        func: 'get_photo_cursor_page',
        steps: [
          { metric: 'find_cursor_page_ids', label: '查询分页 ID' },
          { metric: 'load_photos_info', label: '加载照片信息' },
        ],
      },
      {
        rowTitle: '上传照片 (upload_photo)',
        name: '上传照片',
        func: 'upload_photo',
        steps: [
          { metric: 'validate_photo:duration_seconds', label: '图片校验' },
          { metric: 'md5_hash:duration_seconds', label: 'MD5 计算' },
          { metric: 's3_upload', label: 'S3 上传' },
          { metric: 'db_insert', label: '数据库插入' },
          { metric: 'cache_get_or_load', label: '缓存读取' },
          { metric: 'cache_put', label: '缓存写入' },
          { metric: 'cache_invalidate', label: '缓存失效' },
        ],
      },
      {
        rowTitle: '检查照片去重 (exists_by_md5_batch)',
        name: '检查照片去重',
        func: 'exists_by_md5_batch',
        steps: [],
      },
      {
        rowTitle: '删除照片 (delete_photos)',
        name: '删除照片',
        func: 'delete_photos',
        steps: [
          { metric: 'db_transaction', label: '数据库事务' },
          { metric: 's3_delete_batch', label: 'S3 批量删除' },
          { metric: 'cache_invalidate', label: '缓存失效' },
          { metric: 'cache_invalidate_dimensions', label: '缓存失效 (尺寸)' },
          { metric: 'cache_invalidate_timeline', label: '缓存失效 (时间线)' },
        ],
      },
      {
        rowTitle: '获取图片 (download_image)',
        name: '获取图片',
        func: 'download_image',
        steps: [
          { metric: 's3_download_process', label: 'S3 下载处理' },
          { metric: 's3_download_stream', label: 'S3 流式下载' },
        ],
      },
      {
        rowTitle: '获取收藏夹列表 (get_collection_list)',
        name: '获取收藏夹列表',
        func: 'get_collection_list',
        steps: [
          { metric: 'query_by_user_id', label: '按用户查询' },
        ],
      },
      {
        rowTitle: '创建收藏夹 (create_collection)',
        name: '创建收藏夹',
        func: 'create_collection',
        steps: [
          { metric: 'db_insert', label: '数据库插入' },
        ],
      },
      {
        rowTitle: '修改收藏夹 (update_collection_info)',
        name: '修改收藏夹',
        func: 'update_collection_info',
        steps: [
          { metric: 'db_update', label: '数据库更新' },
        ],
      },
      {
        rowTitle: '删除收藏夹 (delete_collection)',
        name: '删除收藏夹',
        func: 'delete_collection',
        steps: [
          { metric: 'db_transaction', label: '数据库事务' },
        ],
      },
      {
        rowTitle: '获取照片收藏夹 (get_collections_by_photo)',
        name: '获取照片收藏夹',
        func: 'get_collections_by_photo',
        steps: [],
      },
      {
        rowTitle: '获取收藏夹照片 (get_collection_photos)',
        name: '获取收藏夹照片',
        func: 'get_collection_photos',
        steps: [
          { metric: 'query_photo_ids', label: '查询照片 ID' },
          { metric: 'load_photos_info', label: '加载照片信息' },
        ],
      },
      {
        rowTitle: '添加收藏夹照片 (add_collection_photos)',
        name: '添加收藏夹照片',
        func: 'add_collection_photos',
        steps: [
          { metric: 'auth_check', label: '权限校验' },
          { metric: 'db_transaction', label: '数据库事务' },
        ],
      },
      {
        rowTitle: '移除收藏夹照片 (remove_collection_photos)',
        name: '移除收藏夹照片',
        func: 'remove_collection_photos',
        steps: [
          { metric: 'db_transaction', label: '数据库事务' },
        ],
      },
      {
        rowTitle: '发表评论 (publish_comment)',
        name: '发表评论',
        func: 'publish_comment',
        steps: [
          { metric: 'db_transaction', label: '数据库事务' },
        ],
      },
      {
        rowTitle: '获取评论列表 (get_comment_cursor_page)',
        name: '获取评论列表',
        func: 'get_comment_cursor_page',
        steps: [
          { metric: 'query_hot_comments', label: '热门评论查询' },
          { metric: 'query_by_photo_id', label: '评论列表查询' },
          { metric: 'query_is_like', label: '点赞状态查询' },
        ],
      },
      {
        rowTitle: '删除评论 (delete_comment)',
        name: '删除评论',
        func: 'delete_comment',
        steps: [
          { metric: 'db_transaction', label: '数据库事务' },
        ],
      },
      {
        rowTitle: '点赞 (like_comment)',
        name: '点赞',
        func: 'like_comment',
        steps: [
          { metric: 'db_transaction', label: '数据库事务' },
        ],
      },
      {
        rowTitle: '取消点赞 (unlike_comment)',
        name: '取消点赞',
        func: 'unlike_comment',
        steps: [
          { metric: 'db_transaction', label: '数据库事务' },
        ],
      },
      {
        rowTitle: '点赞照片 (like_photo)',
        name: '点赞照片',
        func: 'like_photo',
        steps: [
          { metric: 'db_transaction', label: '数据库事务' },
        ],
      },
      {
        rowTitle: '取消点赞照片 (unlike_photo)',
        name: '取消点赞照片',
        func: 'unlike_photo',
        steps: [
          { metric: 'db_transaction', label: '数据库事务' },
        ],
      },
      {
        rowTitle: '查询点赞照片 (get_user_liked_photos)',
        name: '查询点赞照片',
        func: 'get_user_liked_photos',
        steps: [
          { metric: 'query_ids', label: '查询点赞 ID' },
        ],
      },
      {
        rowTitle: '重命名人物 (rename_person)',
        name: '重命名人物',
        func: 'rename_person',
        steps: [
          { metric: 'db_transaction', label: '数据库事务' },
        ],
      },
      {
        rowTitle: '合并人物 (merge_person)',
        name: '合并人物',
        func: 'merge_person',
        steps: [
          { metric: 'db_transaction', label: '数据库事务' },
          { metric: 'cache_get_or_load', label: '缓存读取' },
        ],
      },
      {
        rowTitle: '获取人物列表 (get_persons)',
        name: '获取人物列表',
        func: 'get_persons',
        steps: [
          { metric: 'query_page', label: '分页查询' },
        ],
      },
      {
        rowTitle: '搜索人物 (search_persons)',
        name: '搜索人物',
        func: 'search_persons',
        steps: [
          { metric: 'query_search', label: '搜索查询' },
        ],
      },
      {
        rowTitle: '修改人脸归属 (change_face_belonging)',
        name: '修改人脸归属',
        func: 'change_face_belonging',
        steps: [],
      },
      {
        rowTitle: '删除人脸 (delete_face)',
        name: '删除人脸',
        func: 'delete_face',
        steps: [],
      },
      {
        rowTitle: '批量删除人脸 (delete_faces_batch)',
        name: '批量删除人脸',
        func: 'delete_faces_batch',
        steps: [],
      },
      {
        rowTitle: '人脸计算 (face_compute)',
        name: '人脸计算',
        func: 'face_compute',
        steps: [
          { metric: 'query', label: '批次查询' },
          { metric: 'download_batch', label: '批量下载' },
          { metric: 'photo_download', label: '单图下载' },
          { metric: 'photo_decode', label: '图片解码' },
          { metric: 'photo_detect', label: '单图检测' },
          { metric: 'insert', label: '写入数据库' },
        ],
      },
      {
        rowTitle: '时间线统计 (get_monthly_stats)',
        name: '时间线统计',
        func: 'get_monthly_stats',
        steps: [
          { metric: 'cache_get_or_load', label: '缓存读取' },
        ],
      },
      {
        rowTitle: '获取照片信息 (get_photo_info)',
        name: '获取照片信息',
        func: 'get_photo_info',
        steps: [],
      },
      {
        rowTitle: '获取人物照片 (get_person_photos)',
        name: '获取人物照片',
        func: 'get_person_photos',
        steps: [
          { metric: 'query_photo_ids', label: '查询照片 ID' },
          { metric: 'load_photos_info', label: '加载照片信息' },
        ],
      },
      {
        rowTitle: '获取照片人脸 (get_faces_by_photo_id)',
        name: '获取照片人脸',
        func: 'get_faces_by_photo_id',
        steps: [],
      },
      {
        rowTitle: '获取未分配人脸照片 (get_unassigned_face_photos)',
        name: '获取未分配人脸照片',
        func: 'get_unassigned_face_photos',
        steps: [
          { metric: 'query_unassigned_face_photo_ids', label: '查询未分配人脸照片 ID' },
          { metric: 'load_photos_info', label: '加载照片信息' },
        ],
      },
      {
        rowTitle: '删除人物 (delete_person)',
        name: '删除人物',
        func: 'delete_person',
        steps: [
          { metric: 'db_transaction', label: '数据库事务' },
        ],
      },
      {
        rowTitle: '人物全量扫描 (person_full_scan)',
        name: '人物全量扫描',
        func: 'person_full_scan',
        steps: [],
      },
      {
        rowTitle: '人物二次聚类 (person_secondary_cluster)',
        name: '人物二次聚类',
        func: 'person_secondary_cluster',
        steps: [],
      },
    ],
  },
}