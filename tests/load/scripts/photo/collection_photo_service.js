// tests/load/scripts/photo/collection_photo_service.js
// 收藏夹-照片关联压测

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';
import { login, getPhotoUserCredentials, authHeaders, BASE_URL } from '../../helpers/common.js';

// 自定义指标
const cpErrorRate = new Rate('collection_photo_errors');
const cpDuration = new Trend('collection_photo_duration');

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
    collection_photo_errors: ['rate<0.01'],
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

  // 获取照片列表，取一个 photo_id
  const photoListRes = http.get(`${BASE_URL}/photo/?size=1`, { headers });
  if (photoListRes.status !== 200 || !photoListRes.json('data') || photoListRes.json('data').length === 0) {
    console.error('No photos available');
    return;
  }
  const photoId = photoListRes.json('data')[0].id;

  // 创建收藏夹
  const createColRes = http.post(`${BASE_URL}/photo/collections/`, JSON.stringify({
    name: `CP Test ${__VU} ${Date.now()}`,
  }), { headers });

  if (createColRes.status !== 200) {
    console.error(`Create collection failed: ${createColRes.body}`);
    return;
  }
  const collectionId = createColRes.json('data.id');

  sleep(0.3);

  // 1. 添加照片到收藏夹
  const addRes = http.post(`${BASE_URL}/photo/collections/${collectionId}/photos`, JSON.stringify({
    photoIds: [photoId],
  }), { headers });

  check(addRes, {
    'add photos to collection status is 200': (r) => r.status === 200,
  });

  cpErrorRate.add(addRes.status !== 200);
  cpDuration.add(addRes.timings.duration);

  sleep(0.3);

  // 2. 查询收藏夹照片列表
  const listRes = http.get(`${BASE_URL}/photo/collections/${collectionId}/photos?size=10`, { headers });

  check(listRes, {
    'list collection photos status is 200': (r) => r.status === 200,
  });

  cpErrorRate.add(listRes.status !== 200);
  cpDuration.add(listRes.timings.duration);

  sleep(0.3);

  // 3. 从收藏夹移除单张照片
  const removeRes = http.del(`${BASE_URL}/photo/collections/${collectionId}/photos/${photoId}`, null, { headers });

  check(removeRes, {
    'remove photo from collection status is 200': (r) => r.status === 200,
  });

  cpErrorRate.add(removeRes.status !== 200);

  sleep(0.3);

  // 清理：删除收藏夹
  http.del(`${BASE_URL}/photo/collections/${collectionId}`, null, { headers });

  sleep(1);
}
