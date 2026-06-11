import http from 'k6/http';
import { check } from 'k6';
import { SharedArray } from 'k6/data';

// 共享测试图片
const testImages = new SharedArray('images', function () {
  return [
    open('../fixtures/test.jpg', 'b'),
  ];
});

export const options = {
  scenarios: {
    upload: {
      executor: 'shared-iterations',
      vus: 20,
      iterations: 200,
      maxDuration: '5m',
    },
  },
  thresholds: {
    http_req_duration: ['p(95)<1000'],  // 上传较慢，放宽到 1s
  },
};

const BASE_URL = __ENV.APP_URL || 'http://localhost:3000';

// 每个 VU 独立的 token
const tokens = {};

function getToken() {
  if (tokens[__VU]) {
    return tokens[__VU];
  }

  // 每个 VU 使用独立用户登录
  const account = `loadtest_photo_${__VU}@test.com`;
  const loginRes = http.post(`${BASE_URL}/login`, JSON.stringify({
    account,
    password: 'Test123456',
  }), {
    headers: { 'Content-Type': 'application/json' },
  });

  if (loginRes.status === 200) {
    tokens[__VU] = loginRes.json('data.accessToken');
  }

  return tokens[__VU];
}

export default function () {
  const token = getToken();
  if (!token) return;

  const headers = { Authorization: `Bearer ${token}` };

  // 上传
  const image = testImages[__VU % testImages.length];
  const uploadRes = http.post(`${BASE_URL}/photo`, {
    file: http.file(image, 'test.jpg', 'image/jpeg'),
  }, { headers });

  check(uploadRes, {
    'upload success': (r) => r.status === 200,
  });

  // 查询
  const queryRes = http.get(`${BASE_URL}/photo?size=20`, { headers });
  check(queryRes, {
    'query success': (r) => r.status === 200,
  });
}
