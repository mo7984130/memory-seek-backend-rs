// tests/load/scripts/photo/collection_service.js
// 收藏夹模块压测

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';
import { login, getPhotoUserCredentials, authHeaders, BASE_URL } from '../../helpers/common.js';

// 自定义指标
const collectionErrorRate = new Rate('collection_errors');
const collectionDuration = new Trend('collection_duration');

export const options = {
  stages: [
    { duration: '30s', target: 10 },
    { duration: '1m', target: 10 },
    { duration: '30s', target: 20 },
    { duration: '1m', target: 20 },
    { duration: '30s', target: 0 },
  ],
  thresholds: {
    http_req_duration: ['p(95)<200'],
    http_req_failed: ['rate<0.01'],
    collection_errors: ['rate<0.01'],
  },
};

export function setup() {
  return {};
}

export default function () {
  const { account, password } = getPhotoUserCredentials(__VU);

  // 登录获取 token
  const loginResult = login(account, password);
  if (!loginResult) return;

  const { uid, token, refreshToken } = loginResult;
  const headers = authHeaders(uid, token);

  // 1. 创建收藏夹
  const createRes = http.post(`${BASE_URL}/photo/collections/`, JSON.stringify({
    name: `Test Collection ${__VU} ${Date.now()}`,
    description: 'LoadTest collection',
  }), { headers });

  check(createRes, {
    'create collection status is 200': (r) => r.status === 200,
  });

  collectionErrorRate.add(createRes.status !== 200);
  collectionDuration.add(createRes.timings.duration);

  if (createRes.status !== 200) {
    console.error(`Create collection failed: ${createRes.body}`);
    return;
  }

  const collectionId = createRes.json('data.id');

  sleep(0.3);

  // 2. 查询收藏夹列表
  const listRes = http.get(`${BASE_URL}/photo/collections/?size=10`, { headers });

  check(listRes, {
    'list collections status is 200': (r) => r.status === 200,
  });

  collectionErrorRate.add(listRes.status !== 200);
  collectionDuration.add(listRes.timings.duration);

  sleep(0.3);

  // 3. 更新收藏夹信息
  if (collectionId) {
    const updateRes = http.patch(`${BASE_URL}/photo/collections/${collectionId}`, JSON.stringify({
      name: `Updated Collection ${__VU}`,
      description: 'Updated description',
    }), { headers });

    check(updateRes, {
      'update collection status is 200': (r) => r.status === 200,
    });

    collectionErrorRate.add(updateRes.status !== 200);
    collectionDuration.add(updateRes.timings.duration);

    sleep(0.3);

    // 4. 删除收藏夹
    const deleteRes = http.del(`${BASE_URL}/photo/collections/${collectionId}`, null, { headers });

    check(deleteRes, {
      'delete collection status is 200': (r) => r.status === 200,
    });

    collectionErrorRate.add(deleteRes.status !== 200);
  }

  sleep(1);
}
