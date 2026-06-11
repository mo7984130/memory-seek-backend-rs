import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';

// 自定义指标
const loginErrorRate = new Rate('login_errors');
const loginDuration = new Trend('login_duration');

export const options = {
  stages: [
    { duration: '30s', target: 50 },   // 逐步加压到 50 用户
    { duration: '1m', target: 50 },    // 保持 50 用户 1 分钟
    { duration: '30s', target: 100 },  // 加压到 100 用户
    { duration: '1m', target: 100 },   // 保持 100 用户
    { duration: '30s', target: 0 },    // 逐步降压
  ],
  thresholds: {
    http_req_duration: ['p(95)<500'],  // 95% 请求 < 500ms
    login_errors: ['rate<0.1'],        // 错误率 < 10%
  },
};

const BASE_URL = __ENV.APP_URL || 'http://localhost:3000';

// 用户池隔离：每个 VU*1000+ITER 生成唯一用户
function getUserCredentials(vuId, iterId) {
  const userId = vuId * 1000 + iterId;
  return {
    account: `loadtest_user_${userId}@test.com`,
    password: 'Test123456',
  };
}

// 预注册用户（setup 阶段只执行一次）
export function setup() {
  const totalUsers = 100 * 1000; // max_vus * max_iterations
  const registered = [];

  for (let i = 0; i < totalUsers; i++) {
    const { account, password } = getUserCredentials(0, i);
    const res = http.post(`${BASE_URL}/register`, JSON.stringify({
      username: `loadtest_${i}`,
      email: account,
      password: password,
      nickname: `Test User ${i}`,
      inviterCode: 'TEST01',
      emailVerifyCode: 'TEST01',
    }), {
      headers: { 'Content-Type': 'application/json' },
    });

    if (res.status === 200) {
      registered.push(account);
    }
  }

  return { registeredCount: registered.length };
}

export default function (data) {
  // 每次迭代使用不同用户，确保 token 唯一
  const { account, password } = getUserCredentials(__VU, __ITER);

  // 登录
  const loginRes = http.post(`${BASE_URL}/login`, JSON.stringify({
    account,
    password,
  }), {
    headers: { 'Content-Type': 'application/json' },
  });

  check(loginRes, {
    'login status is 200': (r) => r.status === 200,
    'login has token': (r) => r.json('data.accessToken') !== undefined,
  });

  loginErrorRate.add(loginRes.status !== 200);
  loginDuration.add(loginRes.timings.duration);

  if (loginRes.status !== 200) {
    console.error(`Login failed for ${account}: ${loginRes.body}`);
    return;
  }

  sleep(1);
}
