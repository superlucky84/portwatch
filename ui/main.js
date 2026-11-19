// 메뉴바 팝오버 화면. 셸이 보낸 스냅샷을 그리고, 고정 명령만 부른다(docs/DESIGN.md "화면과 갱신").
// 외부 문자열은 모두 textContent로 넣는다.

import {
  GROUPS,
  contentSignature,
  describeKey,
  entryUrl,
  formatTime,
  groupOf,
  managerLabel,
  portsText,
} from './view.js';

const tauri = window.__TAURI__;
const invoke = tauri?.core?.invoke;
const listen = tauri?.event?.listen;

const $ = (id) => document.getElementById(id);
const listElement = $('list');
const settingsElement = $('settings');
const warningsElement = $('warnings');
const statusElement = $('status');
const stopResultElement = $('stop-result');

// 웹뷰 저장소는 막혀 있을 수 있으므로 실패를 무시한다.
const saved = {
  read(key, fallback) {
    try {
      const value = JSON.parse(window.localStorage.getItem(key));
      return value ?? fallback;
    } catch {
      return fallback;
    }
  },
  write(key, value) {
    try {
      window.localStorage.setItem(key, JSON.stringify(value));
    } catch {
      // 저장하지 못해도 화면은 동작한다.
    }
  },
};

const ui = {
  snapshot: null,
  signature: '',
  expanded: new Set(saved.read('portwatch.expanded', [])),
  groupOpen: saved.read('portwatch.groupOpen', {}),
  settings: null,
  choices: { open: [], closed: [] },
  defaultSecurityNames: [],
  // 멈추기 진행 상태: { key, id, title, phase: confirm | running | result | force-confirm, report }.
  stop: null,
};

function element(tag, className, text) {
  const node = document.createElement(tag);
  if (className) {
    node.className = className;
  }
  if (text !== undefined) {
    node.textContent = text;
  }
  return node;
}

let statusTimer = 0;
function showStatus(text) {
  statusElement.textContent = text;
  window.clearTimeout(statusTimer);
  statusTimer = window.setTimeout(() => {
    statusTimer = 0;
    statusElement.textContent = ui.snapshot ? `수집 ${ui.snapshot.collectMs} ms` : '';
  }, 2500);
}

async function call(command, args) {
  try {
    return await invoke(command, args);
  } catch (error) {
    showStatus(String(error));
    throw error;
  }
}

function isGroupOpen(group) {
  return ui.groupOpen[group.id] ?? group.open;
}

function actionButton(label, action, key, extra = {}) {
  const button = element('button', 'action', label);
  button.type = 'button';
  button.dataset.action = action;
  button.dataset.key = key;
  Object.assign(button.dataset, extra);
  return button;
}

function detailRow(label, value) {
  const row = element('div', 'detail-row');
  row.append(element('span', 'detail-label', label), element('span', 'detail-value', value));
  return row;
}

function buildDetail(entry) {
  const detail = element('div', 'entry-detail');
  detail.hidden = !ui.expanded.has(entry.key);
  if (entry.project) {
    detail.append(detailRow('프로젝트', entry.project.path));
  }
  if (entry.manager.label) {
    detail.append(detailRow('라벨', entry.manager.label));
  }
  if (entry.processes[0]) {
    detail.append(detailRow('시작', entry.processes[0].startedAt));
  }
  if (entry.protection) {
    detail.append(detailRow('보호', entry.protection.reason));
  }

  const processes = element('ul', 'processes');
  for (const process of entry.processes.slice(0, 8)) {
    const item = element('li', 'process');
    item.title = process.command;
    item.append(
      element('span', 'pid', String(process.pid)),
      element('span', 'user', process.user),
      element('span', 'command', process.command),
    );
    processes.append(item);
  }
  if (entry.processes.length > 8) {
    processes.append(element('li', 'muted', `외 ${entry.processes.length - 8}개`));
  }
  detail.append(processes);

  const stop = ui.stop?.key === entry.key ? ui.stop : null;
  if (stop) {
    detail.append(buildStopBox(entry, stop));
  }

  const actions = element('div', 'buttons');
  if (entry.stopPlan && !entry.protection && !stop) {
    actions.append(actionButton('멈추기', 'stop-ask', entry.key, { id: entry.id }));
  }
  const url = entryUrl(entry);
  if (url) {
    actions.append(
      actionButton('열기', 'open', entry.key, { url }),
      actionButton('URL 복사', 'copy', entry.key, { url }),
    );
  }
  actions.append(
    entry.hidden
      ? actionButton('다시 보이기', 'unhide', entry.key)
      : actionButton('숨기기', 'hide', entry.key),
    entry.pinned
      ? actionButton('고정 해제', 'unpin', entry.key)
      : actionButton('개발 서버로 고정', 'pin', entry.key),
  );
  detail.append(actions);
  return detail;
}

// 멈추기 확인·진행·결과 칸(docs/DESIGN.md "화면").
function buildStopBox(entry, stop) {
  const box = element('div', 'stop-box');
  box.setAttribute('role', 'group');
  box.setAttribute('aria-label', '멈추기');
  const buttons = element('div', 'buttons');
  switch (stop.phase) {
    case 'confirm': {
      const plan = entry.stopPlan;
      box.append(
        detailRow('대상', entry.title),
        detailRow('관리', managerLabel(entry)),
        detailRow('명령', plan?.command ?? ''),
        detailRow('권한', plan?.admin ? '관리자 비밀번호가 필요합니다' : '일반 권한'),
      );
      buttons.append(
        actionButton('멈추기', 'stop-run', entry.key, { kind: 'danger' }),
        actionButton('취소', 'stop-cancel', entry.key),
      );
      break;
    }
    case 'running':
      box.append(element('p', 'stop-message', '멈추는 중…'));
      break;
    case 'force-confirm':
      box.append(
        element('p', 'stop-message', 'SIGKILL로 강제 종료합니다. 저장하지 않은 내용이 사라질 수 있습니다.'),
      );
      buttons.append(
        actionButton('강제 종료', 'force-run', entry.key, { kind: 'danger' }),
        actionButton('취소', 'stop-cancel', entry.key),
      );
      break;
    default: {
      const report = stop.report;
      box.append(element('p', 'stop-message', report.message));
      if (report.command) {
        box.append(element('p', 'stop-command', report.command));
        buttons.append(actionButton('명령 복사', 'copy-command', entry.key, { command: report.command }));
      }
      if (report.canForce) {
        buttons.append(actionButton('강제 종료', 'force-ask', entry.key, { kind: 'danger' }));
      }
      buttons.append(actionButton('닫기', 'stop-cancel', entry.key));
      break;
    }
  }
  if (buttons.childElementCount > 0) {
    box.append(buttons);
  }
  return box;
}

function buildEntry(entry) {
  const article = element('article', 'entry');
  article.dataset.key = entry.key;

  const head = element('button', 'entry-head');
  head.type = 'button';
  head.dataset.action = 'toggle-entry';
  head.dataset.key = entry.key;
  head.setAttribute('aria-expanded', String(ui.expanded.has(entry.key)));

  const badges = element('span', 'badges');
  badges.append(element('span', 'badge', managerLabel(entry)));
  if (entry.manager.runsAsRoot) {
    badges.append(element('span', 'badge root', 'root'));
  }
  if (entry.pinned) {
    badges.append(element('span', 'badge pinned', '고정'));
  }
  head.append(
    element('span', 'title', entry.title),
    element('span', 'ports', portsText(entry)),
    badges,
  );
  article.append(head, buildDetail(entry));
  return article;
}

function buildGroup(group, entries) {
  const section = element('section', 'group');
  section.dataset.group = group.id;
  const open = isGroupOpen(group);

  const head = element('button', 'group-head');
  head.type = 'button';
  head.dataset.action = 'toggle-group';
  head.dataset.group = group.id;
  head.setAttribute('aria-expanded', String(open));
  head.append(
    element('span', 'chevron', open ? '▾' : '▸'),
    element('span', 'group-name', group.name),
    element('span', 'count', String(entries.length)),
  );

  const body = element('div', 'group-body');
  body.hidden = !open;
  body.append(...entries.map(buildEntry));
  section.append(head, body);
  return section;
}

function renderMessage(text) {
  listElement.replaceChildren(element('p', 'empty', text));
}

function renderList() {
  const snapshot = ui.snapshot;
  if (!snapshot) {
    return;
  }
  const groups = GROUPS.map((group) => ({
    group,
    entries: snapshot.entries.filter((entry) => groupOf(entry) === group.id),
  })).filter(({ entries }) => entries.length > 0);

  const scrollTop = listElement.scrollTop;
  if (groups.length === 0) {
    renderMessage('포트를 연 서버가 없습니다.');
  } else {
    listElement.replaceChildren(...groups.map(({ group, entries }) => buildGroup(group, entries)));
  }
  listElement.scrollTop = scrollTop;
}

function showStopResult(stop) {
  stopResultElement.textContent = stop ? `${stop.title}: ${stop.report.message}` : '';
  stopResultElement.hidden = !stop;
}

// 멈추기 명령을 부르고 결과를 그 항목 자리와 목록 위 줄에 남긴다. 화면은 항목 id만 보낸다.
async function runStop(command) {
  const stop = ui.stop;
  ui.stop = { ...stop, phase: 'running' };
  showStopResult(null);
  renderList();
  let report;
  try {
    report = await call(command, { id: stop.id });
  } catch (error) {
    report = { outcome: 'failed', message: String(error), command: null, canForce: false };
  }
  ui.stop = { ...stop, phase: 'result', report };
  showStopResult(ui.stop);
  renderList();
}

function renderWarnings(warnings) {
  warningsElement.replaceChildren(...warnings.map((warning) => element('li', '', warning)));
  warningsElement.hidden = warnings.length === 0;
}

function applySnapshot(snapshot) {
  if (!snapshot) {
    return;
  }
  $('updated').textContent = `${formatTime(snapshot.takenAtMs)} 갱신`;
  renderWarnings(snapshot.warnings);
  const signature = contentSignature(snapshot);
  ui.snapshot = snapshot;
  if (!statusTimer) {
    statusElement.textContent = `수집 ${snapshot.collectMs} ms`;
  }
  if (signature === ui.signature) {
    return;
  }
  ui.signature = signature;
  renderList();
}

function toggleEntry(button) {
  const key = button.dataset.key;
  const open = !ui.expanded.has(key);
  if (open) {
    ui.expanded.add(key);
  } else {
    ui.expanded.delete(key);
  }
  saved.write('portwatch.expanded', [...ui.expanded]);
  button.setAttribute('aria-expanded', String(open));
  button.nextElementSibling.hidden = !open;
}

function toggleGroup(button) {
  const group = GROUPS.find((candidate) => candidate.id === button.dataset.group);
  const open = !isGroupOpen(group);
  ui.groupOpen[group.id] = open;
  saved.write('portwatch.groupOpen', ui.groupOpen);
  button.setAttribute('aria-expanded', String(open));
  button.querySelector('.chevron').textContent = open ? '▾' : '▸';
  button.nextElementSibling.hidden = !open;
}

async function handleListClick(event) {
  const button = event.target.closest('button[data-action]');
  if (!button) {
    return;
  }
  const { action, key, url, id, command } = button.dataset;
  switch (action) {
    case 'stop-ask': {
      const entry = ui.snapshot.entries.find((candidate) => candidate.id === id);
      ui.stop = { key, id, title: entry?.title ?? key, phase: 'confirm', report: null };
      renderList();
      break;
    }
    case 'stop-cancel':
      ui.stop = null;
      renderList();
      break;
    case 'stop-run':
      await runStop('stop_entry');
      break;
    case 'force-ask':
      ui.stop = { ...ui.stop, phase: 'force-confirm' };
      renderList();
      break;
    case 'force-run':
      await runStop('force_stop_entry');
      break;
    case 'copy-command':
      await call('copy_text', { text: command });
      showStatus('명령을 복사했습니다.');
      break;
    case 'toggle-entry':
      toggleEntry(button);
      break;
    case 'toggle-group':
      toggleGroup(button);
      break;
    case 'open':
      await call('open_url', { url });
      break;
    case 'copy':
      await call('copy_text', { text: url });
      showStatus('URL을 복사했습니다.');
      break;
    case 'hide':
    case 'unhide':
      ui.settings = await call('set_hidden', { key, hidden: action === 'hide' });
      renderSettings();
      break;
    case 'pin':
    case 'unpin':
      ui.settings = await call('set_pinned', { key, pinned: action === 'pin' });
      renderSettings();
      break;
    default:
      break;
  }
}

function fillSelect(select, choices, value) {
  select.replaceChildren(
    ...choices.map((seconds) => {
      const option = element('option', '', `${seconds}초마다`);
      option.value = String(seconds);
      option.selected = seconds === value;
      return option;
    }),
  );
}

function renderSettings() {
  const settings = ui.settings;
  if (!settings) {
    return;
  }
  fillSelect($('refresh-open'), ui.choices.open, settings.refreshOpenSecs);
  fillSelect($('refresh-closed'), ui.choices.closed, settings.refreshClosedSecs);
  $('security-names').value = (settings.securityNames ?? ui.defaultSecurityNames).join('\n');

  const kept = [
    ...settings.hidden.map((key) => ({ key, label: '숨김', action: 'unhide', button: '다시 보이기' })),
    ...settings.pinned.map((key) => ({ key, label: '고정', action: 'unpin', button: '고정 해제' })),
  ];
  $('kept-keys').replaceChildren(
    ...(kept.length === 0
      ? [element('li', 'muted', '없습니다.')]
      : kept.map(({ key, label, action, button }) => {
          const item = element('li', 'kept-item');
          item.append(
            element('span', 'badge', label),
            element('span', 'kept-key', describeKey(key)),
            actionButton(button, action, key),
          );
          return item;
        })),
  );
}

async function updateSettings(update) {
  ui.settings = await call('update_settings', { update });
  renderSettings();
  showStatus('설정을 저장했습니다.');
}

function showSettings(open) {
  settingsElement.hidden = !open;
  listElement.hidden = open;
  if (open) {
    renderSettings();
    $('close-settings').focus();
  }
}

function bindEvents() {
  listElement.addEventListener('click', handleListClick);
  $('kept-keys').addEventListener('click', handleListClick);
  $('refresh').addEventListener('click', () => call('refresh_now'));
  $('quit').addEventListener('click', () => call('quit_app'));
  $('open-settings').addEventListener('click', () => showSettings(true));
  $('close-settings').addEventListener('click', () => showSettings(false));

  $('launch-at-login').addEventListener('change', async (event) => {
    const checkbox = event.target;
    try {
      checkbox.checked = await call('set_launch_at_login', { enabled: checkbox.checked });
    } catch {
      checkbox.checked = !checkbox.checked;
    }
  });
  $('refresh-open').addEventListener('change', (event) =>
    updateSettings({ refreshOpenSecs: Number(event.target.value) }),
  );
  $('refresh-closed').addEventListener('change', (event) =>
    updateSettings({ refreshClosedSecs: Number(event.target.value) }),
  );
  $('save-security').addEventListener('click', () => {
    const names = $('security-names')
      .value.split('\n')
      .map((line) => line.trim())
      .filter(Boolean);
    return updateSettings({ securityNames: names });
  });
  $('reset-security').addEventListener('click', () => updateSettings({ resetSecurityNames: true }));

  document.addEventListener('keydown', (event) => {
    if (event.key !== 'Escape') {
      return;
    }
    if (!settingsElement.hidden) {
      showSettings(false);
    } else {
      call('hide_popover');
    }
  });
}

async function init() {
  if (!invoke || !listen) {
    renderMessage('portwatch 앱 안에서 열어야 합니다.');
    return;
  }
  bindEvents();
  await listen('snapshot', (event) => applySnapshot(event.payload));
  const state = await call('get_state');
  ui.settings = state.settings;
  ui.choices = { open: state.openChoices, closed: state.closedChoices };
  ui.defaultSecurityNames = state.defaultSecurityNames;
  $('launch-at-login').checked = state.launchAtLogin;
  renderSettings();
  if (state.snapshot) {
    applySnapshot(state.snapshot);
  } else {
    renderMessage('수집하는 중입니다…');
  }
}

init();
