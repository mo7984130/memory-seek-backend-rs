// tests/load/helpers/common.js
// k6 公共函数

import http from 'k6/http';

// BASE_URL 必须通过 -e BASE_URL=... 显式传入
const BASE_URL = __ENV.BASE_URL;
if (!BASE_URL) {
  throw new Error('BASE_URL is required. Pass via: k6 run -e BASE_URL=http://host:port script.js');
}

// 数据量配置（与 seed.sql 的 psql 变量对齐）
const AUTH_USERS = parseInt(__ENV.AUTH_USERS || '10000');
const PHOTO_USERS = parseInt(__ENV.PHOTO_USERS || '20');

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
 * 生成 auth 测试用户凭据
 * @param {number} vuId - VU ID
 * @returns {{ account: string, password: string }}
 */
export function getTestUserCredentials(vuId) {
  const userId = (vuId % AUTH_USERS) + 1;
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
  const userId = (vuId % PHOTO_USERS) + 1;
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
