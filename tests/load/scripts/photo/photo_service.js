// tests/load/scripts/photo/photo_service.js
// 照片模块压测

import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';
import { SharedArray } from 'k6/data';
import { login, getPhotoUserCredentials, authHeaders, BASE_URL } from '../../helpers/common.js';

// 自定义指标
const uploadErrorRate = new Rate('upload_errors');
const uploadDuration = new Trend('upload_duration');
const queryErrorRate = new Rate('query_errors');
const queryDuration = new Trend('query_duration');

// 共享图片数据
const testImage = new SharedArray('test-image', function () {
  return [open('../../fixtures/test.jpg', 'b')];
});

export const options = {
  stages: [
    { duration: '30s', target: 10 },
    { duration: '1m', target: 10 },
    { duration: '30s', target: 20 },
    { duration: '1m', target: 20 },
    { duration: '30s', target: 0 },
  ],
  thresholds: {
    http_req_duration: ['p(95)<1000'],
    http_req_failed: ['rate<0.01'],
    upload_errors: ['rate<0.01'],
    query_errors: ['rate<0.01'],
  },
};

export function setup() {
  return {};
}

export default function () {
  const { account, password } = getPhotoUserCredentials(__VU);

  // 登录获取 token
  const token = login(account, password);
  if (!token) return;

  const headers = authHeaders(token);

  // 1. 上传照片
  const formData = {
    file: http.file(testImage[0], 'test.jpg', 'image/jpeg'),
  };

  const uploadRes = http.post(`${BASE_URL}/photo/`, formData, {
    headers: { 'Authorization': `Bearer ${token}` },
  });

  check(uploadRes, {
    'upload status is 200': (r) => r.status === 200,
  });

  uploadErrorRate.add(uploadRes.status !== 200);
  uploadDuration.add(uploadRes.timings.duration);

  if (uploadRes.status !== 200) {
    console.error(`Upload failed: ${uploadRes.body}`);
    return;
  }

  sleep(0.5);

  // 2. 查询照片列表
  const listRes = http.get(`${BASE_URL}/photo/?size=20`, { headers });

  check(listRes, {
    'list status is 200': (r) => r.status === 200,
    'list has data': (r) => r.json('data') !== undefined,
  });

  queryErrorRate.add(listRes.status !== 200);
  queryDuration.add(listRes.timings.duration);

  sleep(0.5);

  // 3. 查询时间线统计
  const timelineRes = http.get(`${BASE_URL}/photo/timeline/stats`, { headers });

  check(timelineRes, {
    'timeline stats status is 200': (r) => r.status === 200,
  });

  queryErrorRate.add(timelineRes.status !== 200);
  queryDuration.add(timelineRes.timings.duration);

  sleep(1);
}
