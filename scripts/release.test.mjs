// scripts/release.mjs 테스트(node --test).

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { test } from 'node:test';

import { checkTag, renderCask, versions } from './release.mjs';

const same = { 'package.json': '1.2.3', 'Cargo.toml': '1.2.3', 'src-tauri/tauri.conf.json': '1.2.3' };

test('태그와 세 파일의 버전이 같아야 한다', () => {
  assert.deepEqual(checkTag('v1.2.3', same), []);
  assert.deepEqual(checkTag('v1.2.4', { ...same, 'Cargo.toml': '1.2.4', 'src-tauri/tauri.conf.json': '1.2.4' }), [
    'package.json의 버전 1.2.3이 태그 1.2.4과 다릅니다.',
  ]);
  assert.deepEqual(checkTag('1.2.3', same), ['태그는 v<주>.<부>.<수> 모양이어야 합니다: 1.2.3']);
  assert.deepEqual(checkTag('v1.2', same), ['태그는 v<주>.<부>.<수> 모양이어야 합니다: v1.2']);
});

test('저장소의 버전 세 곳을 읽는다', () => {
  const found = versions();
  assert.equal(new Set(Object.values(found)).size, 1, JSON.stringify(found));
  assert.match(found['Cargo.toml'], /^\d+\.\d+\.\d+$/);
});

test('cask 틀에 버전과 SHA-256만 넣는다', () => {
  const template = readFileSync(join(import.meta.dirname, '..', 'packaging/homebrew/portwatch.rb.in'), 'utf8');
  const sha = 'a'.repeat(64);
  const cask = renderCask(template, '0.2.0', sha);
  assert.match(cask, /^ {2}version "0\.2\.0"$/m);
  assert.match(cask, new RegExp(`^ {2}sha256 "${sha}"$`, 'm'));
  assert.doesNotMatch(cask, /@[A-Z0-9]+@/);
  assert.match(cask, /portwatch-v#\{version\}\/portwatch-#\{version\}-arm64\.zip/);
  assert.throws(() => renderCask(template, '0.2', sha));
  assert.throws(() => renderCask(template, '0.2.0', 'A'.repeat(64)));
  assert.throws(() => renderCask(template, '0.2.0"; system "x', sha));
});
