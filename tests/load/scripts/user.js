// tests/load/scripts/user.js
// 用户模块端到端压测

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';
import { login, getTestUserCredentials, authHeaders, BASE_URL } from '../helpers/common.js';

// 自定义指标
const profileErrorRate = new Rate('profile_errors');
const profileDuration = new Trend('profile_duration');

export const options = {
  stages: [
    { duration: '30s', target: 20 },   // 逐步加压到 20 用户
    { duration: '1m', target: 20 },    // 保持 20 用户 1 分钟
    { duration: '30s', target: 50 },   // 加压到 50 用户
    { duration: '1m', target: 50 },    // 保持 50 用户
    { duration: '30s', target: 0 },    // 逐步降压
  ],
  thresholds: {
    http_req_duration: ['p(95)<200'],  // 95% 请求 < 200ms
    http_req_failed: ['rate<0.01'],    // 错误率 < 1%
    profile_errors: ['rate<0.01'],     // 错误率 < 1%
  },
};

export function setup() {
  return {};
}

export default function (data) {
  const { account, password } = getTestUserCredentials(__VU);

  // 登录获取 token
  const token = login(account, password);
  if (!token) return;

  const headers = authHeaders(token);

  // 获取个人信息
  const getRes = http.get(`${BASE_URL}/user/profile`, { headers });

  check(getRes, {
    'get profile status is 200': (r) => r.status === 200,
    'get profile has data': (r) => r.json('data') !== undefined,
  });

  profileErrorRate.add(getRes.status !== 200);
  profileDuration.add(getRes.timings.duration);

  if (getRes.status !== 200) {
    console.error(`Get profile failed: ${getRes.body}`);
    return;
  }

  sleep(0.5);

  // 更新个人信息
  const updateRes = http.put(`${BASE_URL}/user/profile`, JSON.stringify({
    nickname: `Updated User ${__VU} ${Date.now()}`,
  }), { headers });

  check(updateRes, {
    'update profile status is 200': (r) => r.status === 200,
  });

  profileErrorRate.add(updateRes.status !== 200);
  profileDuration.add(updateRes.timings.duration);

  sleep(1);
}
