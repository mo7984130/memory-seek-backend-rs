// tests/load/scripts/auth.js
// 认证模块端到端压测

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';
import { login, getTestUserCredentials, authHeaders, BASE_URL } from '../helpers/common.js';

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
    http_req_duration: ['p(95)<200'],  // 95% 请求 < 200ms
    http_req_failed: ['rate<0.01'],    // 错误率 < 1%
    login_errors: ['rate<0.01'],       // 登录错误率 < 1%
  },
};

export function setup() {
  // 数据已通过 seed.sql 预置，setup 无需操作
  return {};
}

export default function (data) {
  const { account, password } = getTestUserCredentials(__VU);

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

  const token = loginRes.json('data.accessToken');

  // 访问受保护接口（获取当前用户信息）
  const profileRes = http.get(`${BASE_URL}/user/profile`, {
    headers: authHeaders(token),
  });

  check(profileRes, {
    'profile status is 200': (r) => r.status === 200,
  });

  sleep(1);
}
