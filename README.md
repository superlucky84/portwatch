# portwatch

로컬 개발 서버를 한곳에서 보고 멈추는 macOS 메뉴바 앱입니다.

> 개발 중입니다. 메뉴바 창에서 서버 목록을 보고 숨기기·고정·멈추기를 할 수 있고, nginx 경로·hosts 이름·인증서를 보여줍니다.
> 첫 배포 전 사용자 Mac 확인이 남아 있습니다.

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

## 설치

Apple Silicon Mac, macOS 15 이상에서 Homebrew로 설치합니다.

```bash
brew install --cask superlucky84/tap/portwatch
```

Apple 서명·공증이 없는 앱이라 처음 실행할 때 macOS가 막습니다. 한 번만 허용하면 됩니다.

1. Spotlight나 `/Applications`에서 portwatch를 실행합니다. "확인되지 않은 개발자" 안내가 뜨면 **완료**를 누릅니다.
2. **시스템 설정 → 개인정보 보호 및 보안**을 열고 아래쪽의 **그래도 열기**를 누릅니다.
3. 다시 묻는 창에서 **열기**를 누르고 Mac 암호(또는 Touch ID)로 확인합니다.

업데이트 뒤에는 같은 절차가 다시 필요할 수 있습니다. 회사 보안 프로그램이 서명 없는 앱을 막으면 담당 부서 정책을 따르세요.

업데이트와 삭제:

```bash
brew upgrade --cask portwatch
```

```bash
brew uninstall --cask portwatch
```

설정 파일까지 지우려면 `brew uninstall --zap --cask portwatch`를 씁니다.

### 배포하기 (관리자용)

코드는 비공개로 두고, 빌드한 앱 zip만 공개 저장소 `superlucky84/homebrew-tap`의 Releases와 `Casks/portwatch.rb`에 올립니다(DC-18).
처음 한 번 준비할 것:

- 공개 저장소 `superlucky84/homebrew-tap`을 만듭니다(기본 브랜치에 README 하나면 충분합니다).
- 그 저장소 하나에만 **Contents: Read and write** 권한을 준 fine-grained 토큰을 만들어, portwatch 저장소의
  Actions secret `TAP_TOKEN`으로 넣습니다.

새 버전을 낼 때는 `package.json`·`Cargo.toml`(workspace)·`src-tauri/tauri.conf.json`의 버전을 같이 올려 커밋하고 태그를 푸시합니다.

```bash
git tag v0.1.0
```

```bash
git push origin v0.1.0
```

`Release` workflow가 macOS에서 빌드·ad-hoc 서명·zip을 만들고 tap 저장소에 `portwatch-v<버전>` 릴리스와 cask를 올립니다.
Actions 탭에서 **Release → Run workflow**로 수동 실행하면 리허설로 빌드와 artifact까지만 합니다(`publish`를 켜면 올립니다).
