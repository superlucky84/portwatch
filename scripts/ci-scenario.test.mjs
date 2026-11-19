// scripts/ci-scenario.mjs 테스트(node --test). 실행기에서만 도는 확인 스크립트가 맞게 판정하는지 합성 스냅샷으로 본다.

import assert from 'node:assert/strict';
import { test } from 'node:test';

import { check } from './ci-scenario.mjs';

function snapshot(root) {
  const tls = {
    subject: 'localhost',
    sans: ['localhost'],
    issuer: 'portwatch CI',
    mkcert: false,
    notAfterMs: 0,
    warnings: ['인증서가 14일 안에 만료됩니다.'],
    error: null,
  };
  const domain = (via) => ({
    name: 'localhost',
    via,
    scheme: 'https',
    port: 18443,
    path: '/',
    upstream: 18081,
    inHosts: true,
    tls,
  });
  return {
    entries: [
      {
        listeners: [{ port: 8080 }, { port: 18443 }],
        manager: { kind: 'launchd-brew', domain: root ? 'system' : 'user', formula: 'nginx', runsAsRoot: root },
        protection: null,
        stopPlan: { method: { kind: 'brew-services' }, admin: root },
        domains: [domain('nginx')],
      },
      {
        listeners: [{ port: 18081 }],
        manager: { kind: 'detached' },
        stopPlan: { method: { kind: 'signal' }, admin: false },
        domains: [domain('proxied')],
      },
    ],
    warnings: [],
  };
}

test('기대한 모양이면 문제가 없다', () => {
  assert.deepEqual(check('user-brew', snapshot(false)), []);
  assert.deepEqual(check('root-brew', snapshot(true)), []);
});

test('root 여부와 빠진 항목을 알린다', () => {
  assert.deepEqual(check('root-brew', snapshot(false)), [
    'nginx 도메인: user',
    'nginx root 여부: false',
    'nginx 관리자 권한: false',
  ]);
  const missing = snapshot(false);
  missing.entries.pop();
  assert.deepEqual(check('user-brew', missing), ['18081 항목(분리 실행 node)이 없습니다.']);
});
