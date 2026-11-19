// 화면 테스트용 합성 스냅샷과 가짜 window.__TAURI__.
// 사용자 이름은 dev, 경로는 합성 값이다(docs/MANUAL_TEST_CHECKLIST.md "합성 자료").

function entry(key, title, category, kind, ports, extra = {}) {
  return {
    id: `${kind}:${ports[0]}:Wed Nov 18 09:00:00 2026`,
    key,
    title,
    category,
    manager: {
      kind,
      domain: null,
      label: null,
      formula: null,
      container: null,
      appName: null,
      runsAsRoot: false,
      ...extra.manager,
    },
    listeners: ports.map((port) => ({ port, address: extra.address ?? '*', family: 'tcp4', pid: 2000 + port })),
    processes: extra.processes ?? [
      {
        pid: 2000 + ports[0],
        ppid: 1,
        pgid: 2000 + ports[0],
        uid: 501,
        user: 'dev',
        tty: 'ttys001',
        startedAt: 'Wed Nov 18 09:00:00 2026',
        command: `node server.js --port ${ports[0]}`,
      },
    ],
    project: extra.project ?? null,
    domains: extra.domains ?? [],
    protection: extra.protection ?? null,
    stopPlan: extra.stopPlan ?? null,
    hidden: extra.hidden ?? false,
    pinned: extra.pinned ?? false,
  };
}

// nginx가 443에서 shop-web(5173)으로 넘기는 주소. 인증서는 열흘 뒤 만료라 경고가 붙는다.
function shopDomain(via) {
  return {
    name: 'shop.example.test',
    via,
    scheme: 'https',
    port: 443,
    path: '/',
    upstream: 5173,
    inHosts: true,
    tls: {
      subject: 'shop.example.test',
      sans: ['shop.example.test'],
      issuer: 'mkcert development CA',
      mkcert: true,
      notAfterMs: Date.UTC(2026, 10, 29, 12),
      warnings: ['인증서가 14일 안에 만료됩니다.'],
      error: null,
    },
  };
}

export function snapshot(overrides = {}) {
  return {
    takenAtMs: Date.UTC(2026, 10, 19, 0, 30, 5),
    collectMs: 140,
    warnings: [],
    entries: [
      entry('project:/Users/dev/work/api', 'api-server', 'dev-server', 'terminal', [3000], {
        project: { name: 'api-server', path: '/Users/dev/work/api' },
        address: '127.0.0.1',
      }),
      entry('project:/Users/dev/work/shop-web', 'shop-web', 'dev-server', 'terminal', [5173], {
        project: { name: 'shop-web', path: '/Users/dev/work/shop-web' },
        domains: [shopDomain('proxied')],
        stopPlan: { method: { kind: 'signal', pid: 2100, group: true }, command: 'kill -TERM -2100', admin: false },
        processes: Array.from({ length: 10 }, (_, index) => ({
          pid: 2100 + index,
          ppid: index === 0 ? 2001 : 2100,
          pgid: 2100,
          uid: 501,
          user: 'dev',
          tty: 'ttys001',
          startedAt: 'Wed Nov 18 09:11:00 2026',
          command: index === 0 ? 'npm run dev' : `node worker-${index}.js <b>not html</b>`,
        })),
      }),
      entry('launchd:sh.brew.nginx', 'nginx', 'service', 'launchd-brew', [8080], {
        manager: { domain: 'system', label: 'sh.brew.nginx', formula: 'nginx', runsAsRoot: true },
        domains: [
          {
            name: 'localhost',
            via: 'nginx',
            scheme: 'http',
            port: 8080,
            path: '/api',
            upstream: 3000,
            inHosts: true,
            tls: null,
          },
          {
            name: 'intranet.example.test',
            via: 'nginx',
            scheme: 'http',
            port: 8080,
            path: '/',
            upstream: 3000,
            inHosts: false,
            tls: null,
          },
          shopDomain('nginx'),
        ],
        stopPlan: {
          method: { kind: 'brew-services', brew: '/opt/homebrew/bin/brew', formula: 'nginx', domain: 'system' },
          command: 'brew services stop nginx',
          admin: true,
        },
      }),
      entry('exe:codex', 'codex', 'dev-tool', 'terminal', [53733], { address: '127.0.0.1' }),
      entry('app:Player', 'Player', 'app', 'app', [57621], {
        manager: { appName: 'Player' },
        processes: [
          {
            pid: 700,
            ppid: 1,
            pgid: 700,
            uid: 501,
            user: 'dev',
            tty: '??',
            startedAt: 'Mon Nov 16 08:01:10 2026',
            command: '/Applications/Player.app/Contents/MacOS/Player',
          },
        ],
      }),
      entry('launchd-socket', 'launchd', 'system', 'launchd-socket', [22], {
        manager: { runsAsRoot: true },
        protection: { reason: 'launchd가 직접 연 포트입니다.' },
      }),
      entry('exe:sessiond', 'sessiond', 'dev-tool', 'detached', [2269], { hidden: true }),
    ],
    ...overrides,
  };
}

export function uiState(overrides = {}) {
  return {
    snapshot: snapshot(),
    settings: {
      refreshOpenSecs: 3,
      refreshClosedSecs: 15,
      hidden: ['exe:sessiond'],
      pinned: [],
      securityNames: null,
    },
    launchAtLogin: false,
    openChoices: [2, 3, 5],
    closedChoices: [10, 15, 30, 60],
    defaultSecurityNames: ['veraport', 'crossex', 'ahnlab'],
    ...overrides,
  };
}

// 페이지에 넣는 스크립트. 받은 명령을 window.__calls에 남기고, window.__emit으로 이벤트를 보낸다.
export function installFakeTauri(state) {
  const calls = [];
  const listeners = {};
  let settings = structuredClone(state.settings);
  const toggle = (list, key, on) => [...list.filter((value) => value !== key), ...(on ? [key] : [])];

  window.__calls = calls;
  window.__emit = (name, payload) => (listeners[name] ?? []).forEach((callback) => callback({ payload }));
  window.__TAURI__ = {
    core: {
      async invoke(command, args) {
        calls.push([command, args ?? null]);
        switch (command) {
          case 'get_state':
            return structuredClone(state);
          case 'set_hidden':
            settings = { ...settings, hidden: toggle(settings.hidden, args.key, args.hidden) };
            return settings;
          case 'set_pinned':
            settings = { ...settings, pinned: toggle(settings.pinned, args.key, args.pinned) };
            return settings;
          case 'update_settings': {
            const { update } = args;
            if (update.refreshOpenSecs === 4) {
              throw new Error('창을 연 동안의 갱신 주기로 4초는 고를 수 없습니다.');
            }
            settings = {
              ...settings,
              refreshOpenSecs: update.refreshOpenSecs ?? settings.refreshOpenSecs,
              refreshClosedSecs: update.refreshClosedSecs ?? settings.refreshClosedSecs,
              securityNames: update.resetSecurityNames ? null : (update.securityNames ?? settings.securityNames),
            };
            return settings;
          }
          case 'set_launch_at_login':
            return args.enabled;
          case 'stop_entry':
          case 'force_stop_entry': {
            const reply = (state.stopReplies ?? {})[command];
            if (reply?.error) {
              throw new Error(reply.error);
            }
            return reply ?? { outcome: 'stopped', message: '멈췄습니다.', command: null, canForce: false };
          }
          default:
            return null;
        }
      },
    },
    event: {
      async listen(name, callback) {
        (listeners[name] ??= []).push(callback);
        return () => {};
      },
    },
  };
}
