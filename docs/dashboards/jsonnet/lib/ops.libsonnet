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
//   errors   可选;存在 `{func}:errors:{kind}` 指标时填写 kind 列表,
//            生成器会追加"错误分布"面板
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
          { metric: 'db_query', label: '数据库查询' },
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
          { metric: 'redis_delete', label: 'Redis 删除' },
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
          { metric: 'redis_delete', label: 'Redis 删除' },
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
          { metric: 'db_update', label: '数据库更新' },
        ],
      },
      {
        rowTitle: '登出 (logout)',
        name: '登出',
        func: 'logout',
        steps: [
          { metric: 'db_update', label: '数据库更新' },
          { metric: 'redis_delete', label: 'Redis 删除' },
          { metric: 'redis_delete_cache', label: 'Redis 删除缓存' },
        ],
      },
      {
        rowTitle: '批量获取用户信息 (get_user_info_batch)',
        name: '批量获取用户信息',
        func: 'get_user_info_batch',
        steps: [
          { metric: 'redis_cache', label: 'Redis 缓存' },
        ],
      },
    ],
  },
}