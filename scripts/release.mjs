// 배포 도우미(docs/DESIGN.md "배포").
// 사용법:
//   node scripts/release.mjs check <태그>          태그(v1.2.3)와 package.json·Cargo.toml·tauri.conf.json 버전이 같은지 확인
//   node scripts/release.mjs cask <버전> <sha256>  Homebrew cask를 표준 출력으로 만든다

import { readFileSync } from 'node:fs';
import { join } from 'node:path';

const root = join(import.meta.dirname, '..');
const VERSION = /^\d+\.\d+\.\d+$/;
const SHA256 = /^[0-9a-f]{64}$/;

export function versions(read = (path) => readFileSync(join(root, path), 'utf8')) {
  const cargo = read('Cargo.toml').match(/^\[workspace\.package\][^[]*?^version = "([^"]+)"/m);
  return {
    'package.json': JSON.parse(read('package.json')).version,
    'Cargo.toml': cargo?.[1],
    'src-tauri/tauri.conf.json': JSON.parse(read('src-tauri/tauri.conf.json')).version,
  };
}

export function checkTag(tag, found) {
  const version = tag.replace(/^v/, '');
  if (!tag.startsWith('v') || !VERSION.test(version)) {
    return [`태그는 v<주>.<부>.<수> 모양이어야 합니다: ${tag}`];
  }
  return Object.entries(found)
    .filter(([, value]) => value !== version)
    .map(([file, value]) => `${file}의 버전 ${value ?? '(없음)'}이 태그 ${version}과 다릅니다.`);
}

export function renderCask(template, version, sha256) {
  if (!VERSION.test(version)) {
    throw new Error(`버전 모양이 아닙니다: ${version}`);
  }
  if (!SHA256.test(sha256)) {
    throw new Error(`SHA-256 모양이 아닙니다: ${sha256}`);
  }
  return template.replaceAll('@VERSION@', version).replaceAll('@SHA256@', sha256);
}

function main([command, ...args]) {
  switch (command) {
    case 'check': {
      const problems = checkTag(args[0] ?? '', versions());
      for (const problem of problems) {
        console.error(problem);
      }
      return problems.length === 0 ? 0 : 1;
    }
    case 'cask': {
      const template = readFileSync(join(root, 'packaging/homebrew/portwatch.rb.in'), 'utf8');
      process.stdout.write(renderCask(template, args[0] ?? '', args[1] ?? ''));
      return 0;
    }
    default:
      console.error('사용법: node scripts/release.mjs check <태그> | cask <버전> <sha256>');
      return 2;
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  process.exitCode = main(process.argv.slice(2));
}
