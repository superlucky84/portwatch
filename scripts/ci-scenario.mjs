// macOS CI 실제 서비스 시나리오 확인(Phase 6, docs/IMPLEMENT.md).
// 사용법: node scripts/ci-scenario.mjs <user-brew|root-brew> <probe --json 출력 파일>
// 실행기에서 brew nginx(8080, servers/portwatch.conf의 18443 TLS → 18081)와 분리 실행한 node(18081)를 띄운 뒤 부른다.

import { readFileSync } from 'node:fs';

export function check(scenario, snapshot) {
  const problems = [];
  const expect = (ok, message) => {
    if (!ok) {
      problems.push(message);
    }
  };
  const byPort = (port) => snapshot.entries.find((entry) => entry.listeners.some((l) => l.port === port));

  const nginx = byPort(8080);
  expect(nginx, '8080 항목(nginx)이 없습니다.');
  if (nginx) {
    const root = scenario === 'root-brew';
    expect(nginx.manager.kind === 'launchd-brew', `nginx 관리 주체: ${nginx.manager.kind}`);
    expect(nginx.manager.domain === (root ? 'system' : 'user'), `nginx 도메인: ${nginx.manager.domain}`);
    expect(nginx.manager.formula === 'nginx', `nginx 포뮬러: ${nginx.manager.formula}`);
    expect(nginx.manager.runsAsRoot === root, `nginx root 여부: ${nginx.manager.runsAsRoot}`);
    expect(nginx.protection === null, `nginx 보호: ${JSON.stringify(nginx.protection)}`);
    expect(nginx.stopPlan?.method.kind === 'brew-services', `nginx 멈추기 계획: ${JSON.stringify(nginx.stopPlan)}`);
    expect(nginx.stopPlan?.admin === root, `nginx 관리자 권한: ${nginx.stopPlan?.admin}`);
    expect(nginx.listeners.some((l) => l.port === 18443), 'nginx가 18443을 열지 않았습니다.');

    const tls = nginx.domains.find((d) => d.port === 18443);
    expect(
      tls?.via === 'nginx' && tls.scheme === 'https' && tls.name === 'localhost' && tls.upstream === 18081,
      `nginx 주소: ${JSON.stringify(nginx.domains)}`,
    );
    expect(tls?.tls?.issuer === 'portwatch CI', `인증서 발급자: ${JSON.stringify(tls?.tls)}`);
    expect(tls?.tls?.warnings.includes('인증서가 14일 안에 만료됩니다.'), `인증서 경고: ${JSON.stringify(tls?.tls)}`);
  }

  const node = byPort(18081);
  expect(node, '18081 항목(분리 실행 node)이 없습니다.');
  if (node) {
    expect(node.manager.kind === 'detached', `node 관리 주체: ${node.manager.kind}`);
    expect(node.stopPlan?.method.kind === 'signal', `node 멈추기 계획: ${JSON.stringify(node.stopPlan)}`);
    expect(
      node.domains.some((d) => d.via === 'proxied' && d.port === 18443),
      `node 주소: ${JSON.stringify(node.domains)}`,
    );
  }
  return problems;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const [scenario, file] = process.argv.slice(2);
  if (!['user-brew', 'root-brew'].includes(scenario) || !file) {
    console.error('사용법: node scripts/ci-scenario.mjs <user-brew|root-brew> <json 파일>');
    process.exit(2);
  }
  const snapshot = JSON.parse(readFileSync(file, 'utf8'));
  const problems = check(scenario, snapshot);
  console.log(`${scenario}: 항목 ${snapshot.entries.length}개, 수집 ${snapshot.collectMs} ms, 경고 ${snapshot.warnings.length}개`);
  for (const warning of snapshot.warnings) {
    console.log(`  경고: ${warning}`);
  }
  for (const problem of problems) {
    console.error(`  실패: ${problem}`);
  }
  process.exit(problems.length === 0 ? 0 : 1);
}
