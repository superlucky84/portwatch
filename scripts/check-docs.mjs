// 문서 검사: 상대 링크와 제목 앵커, RQ/PR/DC/M/F 참조가 정의된 ID를 가리키는지 확인한다.

import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';

const root = join(import.meta.dirname, '..');
const skipDirs = new Set(['.git', 'node_modules', 'target', 'gen']);

function markdownFiles(dir) {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    if (entry.isDirectory()) {
      return skipDirs.has(entry.name) ? [] : markdownFiles(join(dir, entry.name));
    }
    return entry.name.endsWith('.md') ? [join(dir, entry.name)] : [];
  });
}

function slug(heading) {
  return heading
    .trim()
    .toLowerCase()
    .replace(/[^\p{L}\p{N}\s_-]/gu, '')
    .replace(/\s/g, '-');
}

const files = markdownFiles(root);
const text = new Map(files.map((file) => [file, readFileSync(file, 'utf8')]));
const read = (path) => text.get(join(root, path)) ?? '';

const defined = new Set();
const addAll = (source, pattern) => {
  for (const match of source.matchAll(pattern)) {
    defined.add(match[1]);
  }
};
addAll(read('docs/REQUIREMENTS.md'), /^\| ((?:RQ|PR)-\d+) \|/gm);
addAll(read('docs/DESIGN.md'), /^- \[[ x]\] (DC-\d+):/gm);
addAll(read('docs/MANUAL_TEST_CHECKLIST.md'), /^### (M-\d+) /gm);
addAll(read('docs/MANUAL_TEST_CHECKLIST.md'), /^\| (F-\d+) \|/gm);

const problems = [];
const id = (prefix, number) => `${prefix}-${String(number).padStart(2, '0')}`;
const requireId = (value, file) => {
  if (!defined.has(value)) {
    problems.push(`${relative(root, file)}: 정의되지 않은 ID ${value}`);
  }
};

for (const [file, source] of text) {
  for (const match of source.matchAll(/\b(RQ|PR|DC|M|F)-(\d+)(?:~(?:\1-)?(\d+))?/g)) {
    requireId(id(match[1], match[2]), file);
    if (match[3]) {
      for (let n = Number(match[2]); n <= Number(match[3]); n += 1) {
        requireId(id(match[1], n), file);
      }
    }
  }
  for (const match of source.matchAll(/\b(RQ|PR|DC|M)-(\d+)((?:·\d+)+)/g)) {
    for (const n of match[3].split('·').filter(Boolean)) {
      requireId(id(match[1], n), file);
    }
  }
  for (const match of source.matchAll(/\]\(([^)\s#]*)(#[^)\s]+)?\)/g)) {
    const [, target, anchor] = match;
    if (/^[a-z]+:/i.test(target)) {
      continue;
    }
    const targetFile = target ? join(dirname(file), target) : file;
    if (!existsSync(targetFile)) {
      problems.push(`${relative(root, file)}: 없는 링크 대상 ${target}`);
      continue;
    }
    if (anchor && targetFile.endsWith('.md')) {
      const headings = [...readFileSync(targetFile, 'utf8').matchAll(/^#+ (.+)$/gm)].map((h) => slug(h[1]));
      if (!headings.includes(decodeURIComponent(anchor.slice(1)))) {
        problems.push(`${relative(root, file)}: 없는 앵커 ${target}${anchor}`);
      }
    }
  }
}

if (problems.length > 0) {
  console.error(problems.join('\n'));
  process.exit(1);
}
console.log(`문서 ${files.length}개, ID ${defined.size}개 확인`);
