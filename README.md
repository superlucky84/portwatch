# portwatch

로컬 개발 서버를 한곳에서 보고 멈추는 macOS 메뉴바 앱입니다.

> 개발 초기 단계입니다. 메뉴바 창에서 서버 목록을 보고 숨기거나 고정할 수 있습니다. 멈추기는 아직 없습니다.

## 무엇을 하나요

- 포트만 다른 localhost 서버 여러 개를 프로젝트별로 보여줍니다.
- `brew services`로 띄운 서비스, `sudo`로 띄운 root 서비스까지 sudo 없이 찾습니다.
- 서버마다 무엇이 관리하는지(터미널, brew 서비스, Docker, pm2 등) 보여주고, 그 방식에 맞게 멈춥니다.
- hosts 파일과 nginx 설정을 읽어 "도메인 → nginx → 개발 서버" 연결과 인증서 만료일을 보여줍니다.
- 보안 프로그램이나 시스템 서비스 같은 잡음은 기본으로 접어 둡니다.

## 문서

이 프로젝트는 문서 기반 개발(Doc-Driven Development)로 진행합니다.

- [요구사항](./docs/REQUIREMENTS.md)
- [설계](./docs/DESIGN.md)
- [구현 계획](./docs/IMPLEMENT.md)
- [수동 검증](./docs/MANUAL_TEST_CHECKLIST.md)

## 개발

Tauri 2로 만듭니다. 탐지와 판별 로직(`crates/portwatch-core`)은 macOS 없이도 테스트할 수 있습니다.

| 위치 | 내용 |
| --- | --- |
| `crates/portwatch-core/` | 명령 출력을 해석하고 판별하는 순수 Rust 코드 |
| `crates/portwatch-collect/` | macOS 명령 실행과 진단용 `portwatch-probe` |
| `src-tauri/` | 메뉴바 아이콘과 팝오버 창 |
| `ui/` | 팝오버 화면(HTML/CSS/JS) |

필요한 것: Rust(stable), Node 22 이상. macOS 앱 빌드에는 Xcode Command Line Tools도 필요합니다.

```bash
npm ci
```

```bash
npm run check
```

```bash
npm test
```

메뉴바 창 화면은 가짜 Tauri 객체를 넣어 Playwright로 테스트합니다(처음 한 번은 `npx playwright install chromium`).

```bash
npm run test:ui
```

macOS에서 앱을 빌드하면 `target/release/bundle/macos/portwatch.app`이 생깁니다.

```bash
npm run build:mac
```

### CI에서 빌드한 앱 실행하기

macOS 빌드는 비용 때문에 수동으로만 돌립니다. GitHub의 Actions 탭에서 **CI → Run workflow**를 실행하면
`portwatch-macos-arm64` artifact(zip)가 생깁니다. 압축을 풀면 `portwatch.app`과 `portwatch-probe`가 있습니다.

Apple 서명이 없어서 첫 실행을 macOS가 막습니다. **시스템 설정 → 개인정보 보호 및 보안**에서 **그래도 열기**를 누르세요.
직접 빌드한 본인 앱이라면 터미널에서 격리 표시를 지워도 됩니다.

```bash
xattr -dr com.apple.quarantine /Applications/portwatch.app
```

실행하면 Dock 아이콘 없이 메뉴바에 고리 모양 아이콘이 생기고, 옆에 개발 서버·서비스 수가 표시됩니다.
왼쪽 클릭은 목록 창, 오른쪽 클릭은 종료 메뉴입니다. 목록 창에서 행을 누르면 상세와 열기·URL 복사·숨기기·고정 버튼이 나오고,
설정에서 갱신 주기·로그인 시 자동 실행·보안 프로그램 목록을 바꿉니다. 설정은
`~/Library/Application Support/io.github.superlucky84.portwatch/settings.json`에 저장됩니다.

### 탐지 결과 확인하기 (`portwatch-probe`)

artifact에는 화면 없이 탐지 결과를 출력하는 `portwatch-probe`도 들어 있습니다. 내려받은 파일이라 격리 표시를 먼저 지웁니다.

```bash
xattr -d com.apple.quarantine ./portwatch-probe
```

분류별 표로 봅니다. 홈 경로는 `~`로 표시합니다.

```bash
./portwatch-probe
```

출력 형식 확인용 원문입니다. 사용자 이름은 `<user>`, 홈 경로는 `~`로 가리고, 포트를 연 프로세스와 그 부모에 관한 줄만 남깁니다.

```bash
./portwatch-probe --raw
```

`--json`은 앱이 받는 스냅샷 JSON을 그대로 출력합니다.

## 계획한 설치 방법

Homebrew cask로 설치할 예정입니다. 배포 위치는 아직 정하지 않았습니다([DC-18](./docs/DESIGN.md#결정-체크리스트)).
Apple 서명 없이 배포하므로 첫 실행 때 macOS의 허용 절차가 필요합니다. 절차는 배포 단계에서 이곳에 적습니다.
