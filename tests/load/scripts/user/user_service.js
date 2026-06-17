// tests/load/scripts/user/user_service.js
// 用户模块压测

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';
import { login, getTestUserCredentials, authHeaders, BASE_URL } from '../../helpers/common.js';

// 自定义指标
const profileErrorRate = new Rate('profile_errors');
const profileDuration = new Trend('profile_duration');
const updateErrorRate = new Rate('update_errors');
const updateDuration = new Trend('update_duration');

export const options = {
  stages: [
    { duration: '30s', target: 20 },
    { duration: '1m', target: 20 },
    { duration: '30s', target: 50 },
    { duration: '1m', target: 50 },
    { duration: '30s', target: 0 },
  ],
  thresholds: {
    http_req_duration: ['p(95)<200'],
    http_req_failed: ['rate<0.01'],
    profile_errors: ['rate<0.01'],
    update_errors: ['rate<0.01'],
  },
};

export function setup() {
  return {};
}

export default function () {
  const { account, password } = getTestUserCredentials(__VU);

  // 登录获取 token
  const loginResult = login(account, password);
  if (!loginResult) return;

  const { uid, token, refreshToken } = loginResult;
  const headers = authHeaders(uid, token);

  // 1. 获取当前用户信息
  const meRes = http.get(`${BASE_URL}/user/me`, { headers });

  check(meRes, {
    'get me status is 200': (r) => r.status === 200,
    'get me has data': (r) => r.json('data') !== undefined,
  });

  profileErrorRate.add(meRes.status !== 200);
  profileDuration.add(meRes.timings.duration);

  if (meRes.status !== 200) {
    console.error(`Get me failed: ${meRes.body}`);
    return;
  }

  sleep(0.5);

  // 2. 修改昵称
  const nicknameRes = http.patch(`${BASE_URL}/user/nickname`, JSON.stringify({
    newNickname: `Updated ${__VU} ${Date.now()}`,
  }), { headers });

  check(nicknameRes, {
    'change nickname status is 200': (r) => r.status === 200,
  });

  updateErrorRate.add(nicknameRes.status !== 200);
  updateDuration.add(nicknameRes.timings.duration);

  sleep(0.5);

  // 3. 修改密码（使用相同密码，避免影响后续测试）
  const passwordRes = http.patch(`${BASE_URL}/user/password`, JSON.stringify({
    oldPassword: password,
    newPassword: password,
  }), { headers });

  check(passwordRes, {
    'change password status is 200': (r) => r.status === 200,
  });

  updateErrorRate.add(passwordRes.status !== 200);
  updateDuration.add(passwordRes.timings.duration);

  sleep(0.5);

  // 4. 登出
  const logoutRes = http.post(`${BASE_URL}/user/logout`, null, { headers });

  check(logoutRes, {
    'logout status is 200': (r) => r.status === 200,
  });

  sleep(1);
}
