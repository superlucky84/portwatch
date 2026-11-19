// 스냅샷 값을 화면 글자로 바꾸는 함수들. DOM을 건드리지 않는다.

export const GROUPS = [
  { id: 'dev-server', name: '개발 서버', open: true },
  { id: 'service', name: '서비스', open: true },
  { id: 'dev-tool', name: '개발 도구', open: false },
  { id: 'app', name: '앱', open: false },
  { id: 'system', name: '시스템·보안', open: false },
  { id: 'hidden', name: '숨긴 항목', open: false },
];

export function groupOf(entry) {
  return entry.hidden ? 'hidden' : entry.category;
}

export function formatTime(ms) {
  const time = new Date(ms);
  return [time.getHours(), time.getMinutes(), time.getSeconds()]
    .map((part) => String(part).padStart(2, '0'))
    .join(':');
}

function topIsApp(entry) {
  return Boolean(entry.processes[0]?.command.includes('.app/Contents/'));
}

export function managerLabel(entry) {
  const manager = entry.manager;
  switch (manager.kind) {
    case 'launchd-brew':
      return 'brew 서비스';
    case 'launchd-other':
      return 'launchd 서비스';
    case 'launchd-socket':
      return 'launchd 소켓';
    case 'docker':
      return 'Docker';
    case 'pm2':
      return 'pm2';
    case 'terminal':
      return '터미널';
    case 'detached':
      return '분리 실행';
    case 'app':
      return topIsApp(entry) ? '앱' : `${manager.appName ?? '앱'}에서 실행`;
    default:
      return '알 수 없음';
  }
}

function isLoopback(address) {
  return address.startsWith('127.') || address === '::1' || address === 'localhost';
}

export function ports(entry) {
  return [...new Set(entry.listeners.map((listener) => listener.port))].sort((a, b) => a - b);
}

export function portsText(entry) {
  const list = ports(entry).map((port) => `:${port}`).join(' ');
  const exposed = entry.listeners.some((listener) => !isLoopback(listener.address));
  return `${list} ${exposed ? '외부' : '로컬'}`;
}

// 열기·복사는 개발 서버와 서비스에만 둔다. 가장 작은 포트를 쓴다.
export function entryUrl(entry) {
  if (!['dev-server', 'service'].includes(entry.category)) {
    return null;
  }
  const [port] = ports(entry);
  return port ? `http://localhost:${port}` : null;
}

// 설정 화면에 보여 줄 key 설명.
export function describeKey(key) {
  const [kind, ...rest] = key.split(':');
  const value = rest.join(':');
  switch (kind) {
    case 'project':
      return `프로젝트 ${value}`;
    case 'launchd':
      return `서비스 ${value}`;
    case 'app':
      return `앱 ${value}`;
    case 'exe':
      return `프로그램 ${value}`;
    default:
      return key;
  }
}

// 수집 시각처럼 매번 바뀌는 값을 뺀 내용 비교용 문자열.
export function contentSignature(snapshot) {
  return JSON.stringify({ ...snapshot, takenAtMs: 0, collectMs: 0 });
}
