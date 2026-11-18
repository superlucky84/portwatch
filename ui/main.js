// 메뉴바 팝오버 화면. 셸이 넘겨준 스냅샷만 그린다(docs/DESIGN.md "스냅샷 계약").

const invoke = window.__TAURI__?.core?.invoke;

const listElement = document.getElementById('list');
const updatedElement = document.getElementById('updated');
const warningsElement = document.getElementById('warnings');

function formatTime(ms) {
  const time = new Date(ms);
  return [time.getHours(), time.getMinutes(), time.getSeconds()]
    .map((part) => String(part).padStart(2, '0'))
    .join(':');
}

function renderMessage(text) {
  listElement.replaceChildren();
  const message = document.createElement('p');
  message.className = 'empty';
  message.textContent = text;
  listElement.append(message);
}

function renderWarnings(warnings) {
  warningsElement.replaceChildren(
    ...warnings.map((warning) => {
      const item = document.createElement('li');
      item.textContent = warning;
      return item;
    }),
  );
  warningsElement.hidden = warnings.length === 0;
}

function renderSnapshot(snapshot) {
  updatedElement.textContent = `${formatTime(snapshot.takenAtMs)} 갱신`;
  renderWarnings(snapshot.warnings);
  if (snapshot.entries.length === 0) {
    renderMessage('아직 표시할 항목이 없습니다. 탐지 기능은 다음 단계에서 추가됩니다.');
    return;
  }
  renderMessage(`항목 ${snapshot.entries.length}개`);
}

async function load() {
  if (!invoke) {
    renderMessage('portwatch 앱 안에서 열어야 합니다.');
    return;
  }
  try {
    renderSnapshot(await invoke('get_snapshot'));
  } catch (error) {
    renderMessage(`목록을 가져오지 못했습니다: ${error}`);
  }
}

document.getElementById('refresh').addEventListener('click', load);
document.getElementById('quit').addEventListener('click', () => invoke?.('quit_app'));
window.addEventListener('focus', load);

load();
