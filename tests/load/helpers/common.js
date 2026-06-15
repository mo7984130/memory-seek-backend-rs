// tests/load/helpers/common.js
// k6 公共函数

import http from 'k6/http';

const BASE_URL = __ENV.BASE_URL || 'http://localhost:3000';

/**
 * 用户登录并返回 accessToken
 * @param {string} account - 邮箱账号
 * @param {string} password - 密码
 * @returns {string|null} accessToken 或 null
 */
export function login(account, password) {
  const res = http.post(`${BASE_URL}/login`, JSON.stringify({
    account,
    password,
  }), {
    headers: { 'Content-Type': 'application/json' },
  });

  if (res.status === 200) {
    return res.json('data.accessToken');
  }

  console.error(`Login failed for ${account}: ${res.status} ${res.body}`);
  return null;
}

/**
 * 生成测试用户凭据
 * @param {number} vuId - VU ID
 * @param {number} maxUsers - 最大用户数
 * @returns {{ account: string, password: string }}
 */
export function getTestUserCredentials(vuId, maxUsers = 1000) {
  const userId = (vuId % maxUsers) + 1;
  return {
    account: `loadtest_${userId}@test.com`,
    password: 'Test123456',
  };
}

/**
 * 生成 photo 测试用户凭据
 * @param {number} vuId - VU ID
 * @returns {{ account: string, password: string }}
 */
export function getPhotoUserCredentials(vuId) {
  const userId = (vuId % 20) + 1;
  return {
    account: `loadtest_photo_${userId}@test.com`,
    password: 'Test123456',
  };
}

/**
 * 创建带 Authorization 头的请求头
 * @param {string} token - accessToken
 * @returns {Object} headers
 */
export function authHeaders(token) {
  return {
    'Content-Type': 'application/json',
    'Authorization': `Bearer ${token}`,
  };
}

export { BASE_URL };
