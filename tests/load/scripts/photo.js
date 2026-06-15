// tests/load/scripts/photo.js
// 照片模块端到端压测

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';
import { SharedArray } from 'k6/data';
import { login, getPhotoUserCredentials, authHeaders, BASE_URL } from '../helpers/common.js';

// 自定义指标
const uploadErrorRate = new Rate('upload_errors');
const uploadDuration = new Trend('upload_duration');
const queryErrorRate = new Rate('query_errors');
const queryDuration = new Trend('query_duration');

// 共享图片数据
const testImage = new SharedArray('test-image', function () {
  return [open('../fixtures/test.jpg', 'b')];
});

export const options = {
  stages: [
    { duration: '30s', target: 10 },   // 逐步加压到 10 用户
    { duration: '1m', target: 10 },    // 保持 10 用户 1 分钟
    { duration: '30s', target: 20 },   // 加压到 20 用户
    { duration: '1m', target: 20 },    // 保持 20 用户
    { duration: '30s', target: 0 },    // 逐步降压
  ],
  thresholds: {
    http_req_duration: ['p(95)<1000'],  // 上传较慢，放宽到 1s
    http_req_failed: ['rate<0.01'],     // 错误率 < 1%
    upload_errors: ['rate<0.01'],       // 上传错误率 < 1%
    query_errors: ['rate<0.01'],        // 查询错误率 < 1%
  },
};

export function setup() {
  return {};
}

export default function (data) {
  const { account, password } = getPhotoUserCredentials(__VU);

  // 登录获取 token
  const token = login(account, password);
  if (!token) return;

  const headers = authHeaders(token);

  // 上传照片
  const formData = {
    file: http.file(testImage[0], 'test.jpg', 'image/jpeg'),
  };

  const uploadRes = http.post(`${BASE_URL}/photo/upload`, formData, {
    headers: {
      'Authorization': `Bearer ${token}`,
    },
  });

  check(uploadRes, {
    'upload status is 200': (r) => r.status === 200,
    'upload has photo id': (r) => r.json('data.id') !== undefined,
  });

  uploadErrorRate.add(uploadRes.status !== 200);
  uploadDuration.add(uploadRes.timings.duration);

  if (uploadRes.status !== 200) {
    console.error(`Upload failed: ${uploadRes.body}`);
    return;
  }

  sleep(0.5);

  // 查询照片列表
  const listRes = http.get(`${BASE_URL}/photo?size=20`, { headers });

  check(listRes, {
    'list status is 200': (r) => r.status === 200,
    'list has data': (r) => r.json('data') !== undefined,
  });

  queryErrorRate.add(listRes.status !== 200);
  queryDuration.add(listRes.timings.duration);

  sleep(0.5);

  // 查询照片详情
  const photoId = uploadRes.json('data.id');
  const detailRes = http.get(`${BASE_URL}/photo/${photoId}`, { headers });

  check(detailRes, {
    'detail status is 200': (r) => r.status === 200,
  });

  queryErrorRate.add(detailRes.status !== 200);
  queryDuration.add(detailRes.timings.duration);

  sleep(1);
}
