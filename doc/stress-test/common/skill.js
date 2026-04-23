import { check, sleep } from 'k6';
import { config } from './config.js';
import {
    ErrorTypes,
    parseJsonSafely,
    makeRequest,
    createErrorResult,
    createSuccessResult,
    logPerformance,
} from './utils.js';

/**
 * ==================== 认证安全规则 ====================
 * 
 * 【重要】Token 失效机制（安全设计）：
 * 
 * 1. 登出后 Token 失效：
 *    - 调用 logout() 后，该用户的所有 accessToken 和 refreshToken 都会失效
 *    - 登出后不能再使用旧 token 进行任何操作
 *    - 测试场景：logoutScenario 中验证登出后获取信息应失败
 * 
 * 2. 重新登录导致旧 Token 失效：
 *    - 同一账号重新登录后，之前的所有 token 都会失效
 *    - 这是安全设计，防止多设备登录
 *    - 并发测试时要注意：多个 VU 登录同一账号会导致 token 竞争
 * 
 * 3. 修改密码后 Token 失效：
 *    - 修改密码后，当前 token 会失效
 *    - 【重要】服务器内部会同步调用登出，清除所有 token
 *    - 需要重新登录获取新 token
 *    - 测试场景：passwordChangeTokenInvalidScenario 中验证修改密码后旧 token 失效
 * 
 * 4. Token 刷新机制：
 *    - 使用 refreshToken 可以获取新的 accessToken
 *    - refreshToken 本身不会因为刷新而失效
 *    - 但重新登录会使旧的 refreshToken 失效
 * 
 * 【测试编写规则】：
 * 
 * 1. 每个场景开始时都要重新登录获取最新 token
 * 2. 登出后不要尝试使用旧 token
 * 3. 修改密码后如需继续操作，必须重新登录
 * 4. 并发场景下，使用 getUserByVU() 分配固定用户，避免 token 竞争
 * 5. 不要在多个操作之间共享 token（除非是测试 token 失效场景）
 * 6. 【重要】修改密码 = 登出，服务器内部会清除所有 token
 * 
 * ==================== 常见问题与解决方案 ====================
 * 
 * 【问题1】HTTP方法不匹配导致405错误
 * 
 * 现象：
 * - logout接口返回405 Method Not Allowed
 * - 测试日志：❌ 登出失败(预期成功): status=405
 * 
 * 原因：
 * - k6测试脚本使用GET方法
 * - 服务器端定义POST方法
 * - HTTP方法不匹配
 * 
 * 解决方案：
 * - 检查服务器端路由定义：.route("/logout", post(Self::logout))
 * - 确保k6脚本使用相同方法：makeRequest('POST', url, null, params)
 * - 建议：所有修改数据的操作都应使用POST/PUT/DELETE，不要使用GET
 * 
 * 【问题2】场景间用户共享导致密码冲突
 * 
 * 现象：
 * - 多个场景使用同一批用户
 * - change_password场景修改了testuser01-08的密码
 * - 其他场景仍然使用旧密码登录testuser01-08，导致"账号或密码错误"
 * - 登录失败耗时2-3ms（快速失败），成功登录耗时5900-6000ms
 * 
 * 原因：
 * - 多个场景共享同一批测试用户
 * - change_password场景修改密码后，只更新了本地JavaScript对象
 * - 其他场景的VU仍然使用旧密码
 * - 数据库中的密码已被修改，但测试脚本不知道
 * 
 * 解决方案：
 * - 使用 getUserByVUAndScenario(vu, scenarioName, scenarioVus) 实现场景级别用户隔离
 * - 每个场景使用独立的用户范围
 * - change_password场景使用testuser16-23
 * - 其他场景使用不同的用户范围
 * - 确保config.testUsers数量 >= 所有场景VU总和
 * 
 * 【问题3】k6 VU隔离导致变量无法共享
 * 
 * 现象：
 * - setup()中初始化的变量无法在VU中访问
 * - 测试日志：⚠️ 场景 "xxx" 未初始化用户范围，使用默认分配
 * 
 * 原因：
 * - k6中每个VU都有独立的JavaScript执行环境
 * - setup()函数只在初始化阶段运行一次
 * - 模块级别的变量（let scenarioUserRanges = {}）不会在VU之间共享
 * - 每个VU执行时，scenarioUserRanges都是初始值{}
 * 
 * 解决方案：
 * - 使用无状态设计，不依赖模块级变量
 * - 将配置作为参数传递给函数
 * - getUserByVUAndScenario(vu, scenarioName, scenarioVus)
 * - 每个VU独立计算用户范围，不依赖共享状态
 * 
 * 【问题4】登出失败导致Token仍然有效
 * 
 * 现象：
 * - logout返回405错误
 * - 使用旧token获取用户信息成功（预期失败）
 * - 测试日志：⚠️ 获取用户信息意外成功(预期失败)
 * 
 * 原因：
 * - logout因HTTP方法错误而失败
 * - Token没有被清除，仍然有效
 * - 后续的"预期失败"测试反而成功
 * 
 * 解决方案：
 * - 先修复logout的HTTP方法问题
 * - 确保logout成功后，再测试token失效
 * - 可以检查logout的返回状态，确保登出成功
 * 
 * 【问题5】密码修改后未恢复，导致测试环境污染
 * 
 * 现象：
 * - 每次运行测试后，测试用户的密码被永久修改
 * - config.testUsers中的初始密码（如123456abc）不再匹配
 * - 后续测试运行失败，提示"账号或密码错误"
 * - 需要手动重置数据库或重新创建测试用户
 * 
 * 原因：
 * - changePassword场景生成随机密码并修改数据库
 * - 只更新了本地JavaScript对象（user.password = newPassword）
 * - 没有将密码恢复到初始值
 * - 数据库中的密码被永久修改
 * 
 * 严重后果：
 * - 测试不可重复运行
 * - 测试环境污染
 * - 需要手动清理
 * 
 * 解决方案：
 * - 在场景结束时将密码恢复到原始值
 * - 保存原始密码：const originalPassword = user.password
 * - 修改密码后，用新密码登录
 * - 使用新token将密码改回：changePassword(userId, newToken, newPassword, originalPassword, false)
 * - 注意：恢复密码是清理操作，expectedSuccess设为false
 * 
 * 代码示例：
 * ```javascript
 * export function changePasswordScenario() {
 *     const user = getUserByVUAndScenario(__VU, 'change_password', scenarioVus);
 *     const loginResult = login(user.account, user.password, true);
 *     
 *     if (!loginResult.success) {
 *         randomSleep();
 *         return;
 *     }
 *     
 *     const originalPassword = user.password;  // 保存原始密码
 *     const newPassword = generateRandomPassword();
 *     
 *     const changePasswordResult = changePassword(
 *         loginResult.userId,
 *         loginResult.accessToken,
 *         originalPassword,
 *         newPassword,
 *         true
 *     );
 *     
 *     if (changePasswordResult.success) {
 *         // ✅ 使用新密码登录，将密码改回原始值
 *         const newLoginResult = login(user.account, newPassword, false);
 *         if (newLoginResult.success) {
 *             changePassword(
 *                 newLoginResult.userId,
 *                 newLoginResult.accessToken,
 *                 newPassword,
 *                 originalPassword,  // 恢复原始密码
 *                 false  // 清理操作，不需要检查结果
 *             );
 *         }
 *     }
 *     
 *     randomSleep();
 * }
 * ```
 * 
 * 【问题6】清理操作输出错误日志，干扰测试结果
 * 
 * 现象：
 * - 密码恢复成功时，输出错误日志：⚠️ 修改密码意外成功(预期失败)
 * - 测试报告显示大量错误，但实际上测试是成功的
 * - 误报率高，影响测试结果判断
 * 
 * 原因：
 * - 密码恢复调用 changePassword(userId, token, newPwd, oldPwd, false)
 * - expectedSuccess = false 表示"预期失败"
 * - 但密码恢复成功时，触发"意外成功"的错误日志
 * - 清理操作不应该使用 expectedSuccess 参数
 * 
 * 解决方案：
 * - 添加 isCleanup 参数区分清理操作
 * - 清理操作成功时不输出错误日志
 * - 清理操作失败时输出警告日志
 * - 清理操作不计入性能统计
 * 
 * 代码示例：
 * ```javascript
 * // 修改 changePassword 函数签名
 * export function changePassword(userId, accessToken, oldPassword, newPassword, expectedSuccess = true, isCleanup = false)
 * 
 * // 清理操作调用
 * changePassword(
 *     newLoginResult.userId,
 *     newLoginResult.accessToken,
 *     newPassword,
 *     originalPassword,
 *     false,  // expectedSuccess（清理操作不关心）
 *     true    // isCleanup = true，标识这是清理操作
 * );
 * ```
 * 
 * 【问题7】密码恢复失败导致测试环境污染
 * 
 * 现象：
 * - 测试日志：❌ 登录失败(预期成功): account=testuser22, status=400, "账号或者密码错误"
 * - 后续测试：❌ 生成邀请码失败(预期成功): status=401, "认证失败"
 * - 数据库中密码被永久修改，config.testUsers中的初始密码不再匹配
 * 
 * 原因：
 * - 密码修改成功后，服务器会清除token（内部调用logout）
 * - 需要用新密码登录获取新token，才能恢复密码
 * - 如果新密码登录失败（网络问题、并发冲突等），密码恢复代码不会执行
 * - 密码被永久修改，无法恢复
 * 
 * 错误代码示例：
 * ```javascript
 * if (changePasswordResult.success) {
 *     const newLoginResult = login(user.account, newPassword, false);
 *     if (newLoginResult.success) {
 *         changePassword(...);  // 恢复密码
 *     }
 *     // ❌ 如果newLoginResult.success是false，密码不会恢复！
 * }
 * ```
 * 
 * 解决方案：
 * - 添加else分支，记录无法恢复的情况
 * - 输出新密码，方便手动恢复
 * - 监控此类错误，及时处理
 * 
 * 正确代码示例：
 * ```javascript
 * if (changePasswordResult.success) {
 *     const newLoginResult = login(user.account, newPassword, false);
 *     if (newLoginResult.success) {
 *         changePassword(
 *             newLoginResult.userId,
 *             newLoginResult.accessToken,
 *             newPassword,
 *             originalPassword,
 *             false,
 *             true
 *         );
 *     } else {
 *         // ✅ 记录无法恢复的情况，输出新密码方便手动恢复
 *         console.error(`⚠️  密码已修改但无法恢复(新密码登录失败): account=${user.account}, newPassword=${newPassword}`);
 *     }
 * }
 * ```
 * 
 * 预防措施：
 * - 为密码修改场景分配独立的用户范围
 * - 定期重置测试用户的密码
 * - 监控"密码已修改但无法恢复"的错误日志
 * - 准备密码重置脚本，快速恢复测试环境
 * 
 * 【问题8】VU编号计算错误导致用户复用
 * 
 * 现象：
 * - 测试日志：❌ 修改密码失败(预期成功): status=400, "原密码错误"
 * - 同一个场景内的多个 VU 使用了相同的用户
 * - VU1 修改了密码，VU9 还在用旧密码操作同一个用户
 * 
 * 原因：
 * - k6 的 VU 编号是全局的（从1开始），不是每个场景独立的
 * - 错误代码：`vuIndex = (vu - 1) % targetRange.vus`
 * - 当 VU 编号超过场景的 VU 数量时，会循环复用用户
 * - 例如：change_password 有 8 个 VU，但 VU9-VU16 会复用 VU1-VU8 的用户
 * 
 * 错误代码：
 * ```javascript
 * const vuIndex = (vu - 1) % targetRange.vus;  // ❌ 错误：会循环复用
 * const userIndex = (targetRange.start + vuIndex) % config.testUsers.length;
 * ```
 * 
 * 正确代码：
 * ```javascript
 * // ✅ 正确：直接使用 VU 编号作为用户索引
 * const userIndex = (targetRange.start + vu - 1) % config.testUsers.length;
 * ```
 * 
 * 用户分配示例：
 * - change_password 场景（8 VUs）：
 *   - VU1 -> testuser16
 *   - VU2 -> testuser17
 *   - ...
 *   - VU8 -> testuser23
 * - password_change_token_invalid 场景（5 VUs）：
 *   - VU1 -> testuser77
 *   - VU2 -> testuser78
 *   - ...
 *   - VU5 -> testuser81
 * 
 * ==================== 并发用户分配规则 ====================
 * 
 * 【重要】避免 Token 竞争：
 * 
 * 问题场景：
 * - VU1 登录 user1 → 获得 token_A
 * - VU2 登录 user1 → 获得 token_B，token_A 失效！
 * - VU1 使用 token_A → 失败！
 * 
 * 解决方案：
 * - 使用 getUserByVUAndScenario(__VU, scenarioName, scenarioVus) 分配场景级别用户
 * - 每个场景使用不同的用户范围
 * - 每个 VU 使用不同的测试账号
 * 
 * 用户数量要求：
 * - config.testUsers.length >= 所有场景VU总和
 * - 否则会出现用户复用，导致 token 竞争
 * 
 * ==================== 场景模板 ====================
 * 
 * 【标准场景模板（推荐）】：
 * ```javascript
 * const scenarioConfigs = {
 *     my_scenario: {
 *         executor: 'constant-vus',
 *         vus: 10,
 *         duration: '5m',
 *         exec: 'myScenario',
 *         tags: { scenario: 'my_scenario' },
 *     },
 * };
 * 
 * // 【重要】定义场景VU数量映射，用于用户隔离
 * const scenarioVus = {
 *     'my_scenario': 10,
 *     // 其他场景...
 * };
 * 
 * export function myScenario() {
 *     const user = getUserByVUAndScenario(__VU, 'my_scenario', scenarioVus);
 *     const loginResult = login(user.account, user.password, true);
 *     
 *     if (!loginResult.success) {
 *         randomSleep();
 *         return;
 *     }
 *     
 *     const result = someOperation(loginResult.userId, loginResult.accessToken, true);
 *     check(result, { '操作成功': (r) => r.success === true });
 *     
 *     randomSleep();
 * }
 * ```
 * 
 * 【Token 失效测试模板】：
 * ```javascript
 * export function tokenInvalidScenario() {
 *     const user = getUserByVUAndScenario(__VU, 'token_invalid', scenarioVus);
 *     const loginResult = login(user.account, user.password, true);
 *     if (!loginResult.success) return;
 *     
 *     // 执行使 token 失效的操作（如登出或修改密码）
 *     const logoutResult = logout(loginResult.userId, loginResult.accessToken, true);
 *     
 *     // 确保登出成功
 *     if (!logoutResult.success) {
 *         console.error('登出失败，无法测试token失效');
 *         return;
 *     }
 *     
 *     // 尝试使用旧 token（预期失败）
 *     const result = someOperation(loginResult.userId, loginResult.accessToken, false);
 *     check(result, { '旧Token应失效': (r) => r.success === false });
 * }
 * ```
 */

export const AuthSecurityRules = {
    TOKEN_INVALID_ON_LOGOUT: true,
    TOKEN_INVALID_ON_RELOGIN: true,
    TOKEN_INVALID_ON_PASSWORD_CHANGE: true,
    SINGLE_SESSION_PER_USER: true,
};

export function createScenario(name, executor, vus, duration, exec, tags) {
    return {
        executor: executor || 'constant-vus',
        vus,
        duration,
        exec,
        tags: tags || { scenario: name },
    };
}

export function createThresholds(scenarioName, p95Threshold, p99Threshold, failRate) {
    return {
        [`http_req_duration{scenario:${scenarioName}}`]: [`p(95)<${p95Threshold}`, `p(99)<${p99Threshold}`],
        [`http_req_failed{scenario:${scenarioName}}`]: [`rate<${failRate}`],
    };
}

export function extractTraceId(res) {
    return res.headers['X-Trace-Id'] || res.headers['x-trace-id'] || 'N/A';
}

export function logRequestError(operation, context, res, expectedSuccess = true) {
    const duration = res.timings.duration;
    const traceId = extractTraceId(res);
    const errorBody = res.body ? res.body.substring(0, 500) : 'N/A';
    
    if (expectedSuccess) {
        console.error(`❌ ${operation}失败(预期成功): ${context}, status=${res.status}, duration=${duration}ms, trace_id=${traceId}, response=${errorBody}`);
        logPerformance(operation, duration, false);
    } else {
        console.error(`⚠️  ${operation}意外成功(预期失败): ${context}, status=${res.status}`);
    }
}

export function handleRequestError(operation, context, error, expectedSuccess = true) {
    if (expectedSuccess) {
        console.error(`❌ ${operation}请求异常: ${context}, error=${error.message}`);
    }
    return createErrorResult(ErrorTypes.REQUEST_FAILED, {
        message: error.message,
        ...context,
    });
}

export function validateResponse(operation, res, expectedSuccess = true) {
    const duration = res.timings.duration;
    const isSuccess = res.status === 200;
    const traceId = extractTraceId(res);

    if (expectedSuccess && !isSuccess) {
        const errorBody = res.body ? res.body.substring(0, 500) : 'N/A';
        console.error(`❌ ${operation}失败(预期成功): status=${res.status}, duration=${duration}ms, trace_id=${traceId}, response=${errorBody}`);
        logPerformance(operation, duration, false);
    } else if (!expectedSuccess && isSuccess) {
        console.error(`⚠️  ${operation}意外成功(预期失败): status=${res.status}`);
    }

    return { isSuccess, duration, traceId };
}

export function parseAndValidate(operation, res, expectedSuccess = true) {
    const { isSuccess, duration, traceId } = validateResponse(operation, res, expectedSuccess);

    if (!isSuccess) {
        return {
            success: false,
            error: ErrorTypes.CHECK_FAILED,
            status: res.status,
            duration,
        };
    }

    const body = parseJsonSafely(res.body, operation);
    if (!body) {
        if (expectedSuccess) {
            console.error(`❌ ${operation}JSON解析失败`);
        }
        return {
            success: false,
            error: ErrorTypes.INVALID_JSON,
            status: res.status,
        };
    }

    logPerformance(operation, duration, true);
    return {
        success: true,
        data: body.data,
        body,
    };
}

export function getRandomUser() {
    const userIndex = Math.floor(Math.random() * config.testUsers.length);
    return config.testUsers[userIndex];
}

/**
 * ==================== 场景级别用户隔离 ====================
 * 
 * 【重要】不同场景使用不同的用户范围，避免Token竞争：
 * 
 * 问题场景：
 * - 场景A修改了user1的密码
 * - 场景B同时使用user1，密码不匹配导致登录失败
 * 
 * 解决方案：
 * - 使用 getUserByVUAndScenario(vu, scenarioName, scenarioVus) 获取用户
 * - 每个场景获得独立的用户范围
 * - 不依赖模块级变量，每个VU独立计算
 */

/**
 * 根据场景名称和VU编号获取用户
 * @param {number} vu - VU编号
 * @param {string} scenarioName - 场景名称
 * @param {Object} scenarioVus - 场景VU数量映射 {scenarioName: vus}
 * @returns {Object} 用户对象
 */
export function getUserByVUAndScenario(vu, scenarioName, scenarioVus = null) {
    if (!scenarioVus) {
        console.warn(`⚠️  未提供场景VU配置，使用默认分配`);
        return getUserByVU(vu);
    }
    
    const sortedScenarios = Object.keys(scenarioVus).sort();
    let offset = 0;
    let targetRange = null;
    
    for (const name of sortedScenarios) {
        const vus = scenarioVus[name] || 0;
        if (name === scenarioName) {
            targetRange = { start: offset, vus: vus };
            break;
        }
        offset += vus;
    }
    
    if (!targetRange || targetRange.vus === 0) {
        console.warn(`⚠️  场景 "${scenarioName}" 未找到VU配置，使用默认分配`);
        return getUserByVU(vu);
    }
    
    // 修复：直接使用 VU 编号作为用户索引，避免循环复用
    // k6 的 VU 编号是全局的，每个 VU 应该使用不同的用户
    const userIndex = (targetRange.start + vu - 1) % config.testUsers.length;
    
    return config.testUsers[userIndex];
}

/**
 * 根据VU编号获取用户（向后兼容，不推荐使用）
 * @deprecated 请使用 getUserByVUAndScenario(vu, scenarioName)
 * @param {number} vu - VU编号
 * @returns {Object} 用户对象
 */
export function getUserByVU(vu) {
    const userIndex = (vu - 1) % config.testUsers.length;
    return config.testUsers[userIndex];
}

export function randomSleep(min = 0.5, max = 1.5) {
    sleep(Math.random() * (max - min) + min);
}

export function buildScenarioOptions(scenarios, baseThresholds = {}) {
    const options = {
        scenarios: {},
        thresholds: {
            checks: ['rate>0.95'],
        },
    };

    for (const [name, scenario] of Object.entries(scenarios)) {
        options.scenarios[name] = scenario;
    }

    Object.assign(options.thresholds, baseThresholds);

    return options;
}

export function validateUserCount(totalVus) {
    const userCount = config.testUsers.length;
    
    if (userCount < totalVus) {
        console.warn('');
        console.warn('⚠️  ==================== 用户数量警告 ====================');
        console.warn(`⚠️  测试用户数量 (${userCount}) < 总 VU 数 (${totalVus})`);
        console.warn(`⚠️  这将导致用户复用，可能引发 Token 竞争问题！`);
        console.warn(`⚠️  建议增加 config.testUsers 数量到至少 ${totalVus} 个`);
        console.warn('⚠️  ========================================================');
        console.warn('');
        return false;
    }
    
    console.log(`✅ 用户数量检查通过: ${userCount} 个用户 >= ${totalVus} 个 VU`);
    return true;
}

export const CommonScenarios = {
    normal: (vus, duration, exec) => createScenario('normal', 'constant-vus', vus, duration, exec, { scenario: 'normal' }),
    error: (vus, duration, exec) => createScenario('error', 'constant-vus', vus, duration, exec, { scenario: 'error' }),
    edge: (vus, duration, exec) => createScenario('edge', 'constant-vus', vus, duration, exec, { scenario: 'edge' }),
};

export const CommonThresholds = {
    normal: { p95: 500, p99: 1000, failRate: 0.01 },
    fast: { p95: 300, p99: 500, failRate: 0.01 },
    strict: { p95: 200, p99: 300, failRate: 0.01 },
};
