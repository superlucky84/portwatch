// 앱 아이콘 원본과 메뉴바 아이콘(template)을 그리고, Tauri CLI로 번들용 크기를 만든다.
// 외부 이미지 도구 없이 Node 내장 zlib로 PNG를 쓴다. 다시 그릴 때만 실행한다: npm run icons

import { spawnSync } from 'node:child_process';
import { readdirSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { deflateSync } from 'node:zlib';

const iconDir = join(import.meta.dirname, '..', 'src-tauri', 'icons');
const keep = new Set([
  'app-icon.png',
  'tray-template.png',
  '32x32.png',
  '128x128.png',
  '128x128@2x.png',
  'icon.icns',
  'icon.png',
]);

const crcTable = Array.from({ length: 256 }, (_, n) => {
  let c = n;
  for (let k = 0; k < 8; k += 1) {
    c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
  }
  return c >>> 0;
});

function crc32(buffer) {
  let c = 0xffffffff;
  for (const byte of buffer) {
    c = crcTable[(c ^ byte) & 0xff] ^ (c >>> 8);
  }
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length);
  const body = Buffer.concat([Buffer.from(type, 'ascii'), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body));
  return Buffer.concat([length, body, crc]);
}

function encodePng(size, rgba) {
  const header = Buffer.alloc(13);
  header.writeUInt32BE(size, 0);
  header.writeUInt32BE(size, 4);
  header[8] = 8; // 비트 깊이
  header[9] = 6; // RGBA
  const rows = Buffer.alloc((size * 4 + 1) * size);
  for (let y = 0; y < size; y += 1) {
    rows[y * (size * 4 + 1)] = 0; // 필터 없음
    rgba.copy(rows, y * (size * 4 + 1) + 1, y * size * 4, (y + 1) * size * 4);
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk('IHDR', header),
    chunk('IDAT', deflateSync(rows, { level: 9 })),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

// paint(x, y)는 [r, g, b, a] 또는 null을 돌려준다. 좌표는 0~1로 정규화한다.
function render(size, paint, samples = 4) {
  const rgba = Buffer.alloc(size * size * 4);
  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      let r = 0;
      let g = 0;
      let b = 0;
      let a = 0;
      for (let sy = 0; sy < samples; sy += 1) {
        for (let sx = 0; sx < samples; sx += 1) {
          const color = paint((x + (sx + 0.5) / samples) / size, (y + (sy + 0.5) / samples) / size);
          if (color) {
            const alpha = color[3] / 255;
            r += color[0] * alpha;
            g += color[1] * alpha;
            b += color[2] * alpha;
            a += alpha;
          }
        }
      }
      const offset = (y * size + x) * 4;
      const total = samples * samples;
      rgba[offset] = a ? Math.round(r / a) : 0;
      rgba[offset + 1] = a ? Math.round(g / a) : 0;
      rgba[offset + 2] = a ? Math.round(b / a) : 0;
      rgba[offset + 3] = Math.round((a / total) * 255);
    }
  }
  return encodePng(size, rgba);
}

function insideRoundedSquare(x, y, min, max, radius) {
  const cx = Math.min(Math.max(x, min + radius), max - radius);
  const cy = Math.min(Math.max(y, min + radius), max - radius);
  return (x - cx) ** 2 + (y - cy) ** 2 <= radius ** 2;
}

// 레이더처럼 고리와 가운데 점: "포트를 지켜본다"는 뜻.
function ringAndDot(x, y, outer, thickness, dot) {
  const distance = Math.hypot(x - 0.5, y - 0.5);
  return (distance <= outer && distance >= outer - thickness) || distance <= dot;
}

const appIcon = render(1024, (x, y) => {
  if (!insideRoundedSquare(x, y, 0.098, 0.902, 0.18)) {
    return null;
  }
  if (ringAndDot(x, y, 0.29, 0.065, 0.085)) {
    return [255, 255, 255, 255];
  }
  const t = (y - 0.098) / 0.804;
  return [Math.round(42 - 19 * t), Math.round(127 - 52 * t), Math.round(143 - 56 * t), 255];
});

const trayIcon = render(44, (x, y) => (ringAndDot(x, y, 0.36, 0.085, 0.12) ? [0, 0, 0, 255] : null), 8);

writeFileSync(join(iconDir, 'app-icon.png'), appIcon);
writeFileSync(join(iconDir, 'tray-template.png'), trayIcon);

const result = spawnSync('npx', ['tauri', 'icon', join(iconDir, 'app-icon.png'), '--output', iconDir], {
  stdio: 'inherit',
});
if (result.status !== 0) {
  process.exit(result.status ?? 1);
}

for (const name of readdirSync(iconDir)) {
  if (!keep.has(name)) {
    rmSync(join(iconDir, name), { recursive: true, force: true });
  }
}
console.log(`아이콘을 만들었습니다: ${[...keep].join(', ')}`);
