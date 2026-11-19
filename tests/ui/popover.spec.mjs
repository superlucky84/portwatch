// 메뉴바 팝오버 화면 테스트(Phase 2, docs/IMPLEMENT.md). 가짜 Tauri로 셸 없이 화면만 검증한다.

import { expect, test } from '@playwright/test';

import { installFakeTauri, snapshot, uiState } from './fake-tauri.mjs';

async function open(page, state = uiState()) {
  await page.addInitScript(installFakeTauri, state);
  await page.goto('/index.html');
  await expect(page.locator('.group').first()).toBeVisible();
}

const calls = (page) => page.evaluate(() => window.__calls);
const group = (page, id) => page.locator(`.group[data-group="${id}"]`);
const entry = (page, key) => page.locator(`.entry[data-key="${key}"]`);

test('개발 서버와 서비스만 펼치고 나머지 분류는 접는다', async ({ page }) => {
  await open(page);
  await expect(page.locator('.group-name')).toHaveText([
    '개발 서버',
    '서비스',
    '개발 도구',
    '앱',
    '시스템·보안',
    '숨긴 항목',
  ]);
  await expect(group(page, 'dev-server').locator('.group-body')).toBeVisible();
  await expect(group(page, 'service').locator('.group-body')).toBeVisible();
  await expect(group(page, 'dev-tool').locator('.group-body')).toBeHidden();
  await expect(group(page, 'hidden').locator('.count')).toHaveText('1');

  const nginx = entry(page, 'launchd:sh.brew.nginx');
  await expect(nginx.locator('.title')).toHaveText('nginx');
  await expect(nginx.locator('.ports')).toHaveText(':8080 외부');
  await expect(nginx.locator('.badge')).toHaveText(['brew 서비스', 'root']);
  await expect(entry(page, 'project:/Users/dev/work/api').locator('.ports')).toHaveText(':3000 로컬');

  await group(page, 'dev-tool').locator('.group-head').click();
  await expect(group(page, 'dev-tool').locator('.group-body')).toBeVisible();
  await expect(group(page, 'dev-tool').locator('.group-head')).toHaveAttribute('aria-expanded', 'true');

  await expect(page.locator('#updated')).toContainText('갱신');
  await expect(page.locator('#status')).toHaveText('수집 140 ms');
});

test('행을 펼치면 상세가 보이고 바깥 문자열은 글자로만 들어간다', async ({ page }) => {
  await open(page);
  const web = entry(page, 'project:/Users/dev/work/shop-web');
  await web.locator('.entry-head').click();
  await expect(web.locator('.entry-detail')).toBeVisible();
  await expect(web.locator('.detail-row').first()).toHaveText('프로젝트/Users/dev/work/shop-web');
  await expect(web.locator('.process')).toHaveCount(8);
  await expect(web.locator('.processes')).toContainText('외 2개');
  await expect(web.locator('.process .command').nth(1)).toHaveText('node worker-1.js <b>not html</b>');
  await expect(web.locator('.process b')).toHaveCount(0);

  const launchd = entry(page, 'launchd-socket');
  await group(page, 'system').locator('.group-head').click();
  await launchd.locator('.entry-head').click();
  await expect(launchd.locator('.entry-detail')).toContainText('launchd가 직접 연 포트입니다.');
  await expect(launchd.getByRole('button', { name: '열기' })).toHaveCount(0);
});

test('열기·URL 복사·숨기기·고정은 정해진 명령만 부른다', async ({ page }) => {
  await open(page);
  const web = entry(page, 'project:/Users/dev/work/shop-web');
  await web.locator('.entry-head').click();
  await web.getByRole('button', { name: '열기' }).click();
  await web.getByRole('button', { name: 'URL 복사' }).click();
  await expect(page.locator('#status')).toHaveText('URL을 복사했습니다.');
  await web.getByRole('button', { name: '숨기기' }).click();
  await web.getByRole('button', { name: '개발 서버로 고정' }).click();

  const recorded = (await calls(page)).filter(([command]) => command !== 'get_state');
  expect(recorded).toEqual([
    ['open_url', { url: 'http://localhost:5173' }],
    ['copy_text', { text: 'http://localhost:5173' }],
    ['set_hidden', { key: 'project:/Users/dev/work/shop-web', hidden: true }],
    ['set_pinned', { key: 'project:/Users/dev/work/shop-web', pinned: true }],
  ]);
});

test('갱신 때 펼친 행과 스크롤을 유지하고 내용이 같으면 다시 그리지 않는다', async ({ page }) => {
  await open(page);
  await group(page, 'dev-tool').locator('.group-head').click();
  await group(page, 'system').locator('.group-head').click();
  const web = entry(page, 'project:/Users/dev/work/shop-web');
  await web.locator('.entry-head').click();
  await page.locator('#list').evaluate((list) => {
    list.scrollTop = 120;
  });
  await page.evaluate(() => {
    window.__marker = document.querySelector('.entry');
  });

  // 수집 시각만 바뀐 스냅샷: 다시 그리지 않는다.
  const same = snapshot({ takenAtMs: Date.UTC(2026, 10, 19, 0, 30, 8), collectMs: 90 });
  await page.evaluate((payload) => window.__emit('snapshot', payload), same);
  expect(await page.evaluate(() => window.__marker === document.querySelector('.entry'))).toBe(true);

  // 항목이 늘어난 스냅샷: 다시 그리되 펼친 행·분류·스크롤을 유지한다.
  const changed = snapshot();
  changed.entries.push({
    ...changed.entries[0],
    id: 'terminal:4000:x',
    key: 'project:/Users/dev/work/admin',
    title: 'admin',
    listeners: [{ port: 4000, address: '*', family: 'tcp4', pid: 4000 }],
  });
  await page.evaluate((payload) => window.__emit('snapshot', payload), changed);
  expect(await page.evaluate(() => window.__marker === document.querySelector('.entry'))).toBe(false);
  await expect(entry(page, 'project:/Users/dev/work/admin')).toBeVisible();
  await expect(web.locator('.entry-detail')).toBeVisible();
  await expect(group(page, 'dev-tool').locator('.group-body')).toBeVisible();
  expect(await page.locator('#list').evaluate((list) => list.scrollTop)).toBe(120);
});

test('경고를 보여 준다', async ({ page }) => {
  await open(page, uiState({ snapshot: snapshot({ warnings: ['작업 폴더(lsof)를 읽지 못했습니다.'] }) }));
  await expect(page.locator('#warnings')).toBeVisible();
  await expect(page.locator('#warnings li')).toHaveText(['작업 폴더(lsof)를 읽지 못했습니다.']);
});

test('설정 화면에서 주기·자동 실행·보안 목록·숨긴 항목을 바꾼다', async ({ page }) => {
  await open(page);
  await page.getByRole('button', { name: '설정' }).click();
  await expect(page.locator('#settings')).toBeVisible();
  await expect(page.locator('#list')).toBeHidden();
  await expect(page.locator('#security-names')).toHaveValue('veraport\ncrossex\nahnlab');
  await expect(page.locator('#kept-keys')).toContainText('프로그램 sessiond');

  await page.locator('#refresh-open').selectOption('5');
  await page.locator('#refresh-closed').selectOption('60');
  await page.locator('#launch-at-login').check();
  await page.locator('#security-names').fill('MyGuard\n\n  other  ');
  await page.getByRole('button', { name: '저장' }).click();
  await expect(page.locator('#security-names')).toHaveValue('MyGuard\nother');
  await page.getByRole('button', { name: '기본값으로' }).click();
  await expect(page.locator('#security-names')).toHaveValue('veraport\ncrossex\nahnlab');
  await page.locator('#kept-keys').getByRole('button', { name: '다시 보이기' }).click();
  await expect(page.locator('#kept-keys')).toHaveText('없습니다.');

  const recorded = (await calls(page)).filter(([command]) => command !== 'get_state');
  expect(recorded).toEqual([
    ['update_settings', { update: { refreshOpenSecs: 5 } }],
    ['update_settings', { update: { refreshClosedSecs: 60 } }],
    ['set_launch_at_login', { enabled: true }],
    ['update_settings', { update: { securityNames: ['MyGuard', 'other'] } }],
    ['update_settings', { update: { resetSecurityNames: true } }],
    ['set_hidden', { key: 'exe:sessiond', hidden: false }],
  ]);

  await page.keyboard.press('Escape');
  await expect(page.locator('#settings')).toBeHidden();
  await expect(page.locator('#list')).toBeVisible();
});

test('Esc로 창을 닫는다', async ({ page }) => {
  await open(page);
  await page.keyboard.press('Escape');
  expect((await calls(page)).map(([command]) => command)).toContain('hide_popover');
});

test('셸 밖에서 열면 안내만 보인다', async ({ page }) => {
  await page.goto('/index.html');
  await expect(page.locator('.empty')).toHaveText('portwatch 앱 안에서 열어야 합니다.');
});
