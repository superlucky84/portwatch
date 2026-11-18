# portwatch 구현 계획

문서 기준: 2026-11-18.
[요구사항](./REQUIREMENTS.md) · [설계](./DESIGN.md) · [수동 검증](./MANUAL_TEST_CHECKLIST.md)

## 현재 상태와 작업 순서

- 완료(2026-11-18): 사용자 Mac에서 sudo 없이 root 서비스를 탐지할 수 있는지 시험했다(요구사항의 확인된 사실).
- 완료(2026-11-18): 기존 도구 검토. 사용자가 LeftOpen을 직접 시험했고 root로 띄운 서비스를 잡지 못했다.
- 완료(2026-11-18): 계획 문서 4종 초안. 사용자 결정 DC-01(Tauri)·DC-02(서명 없이 시작)·DC-04~06을 반영했다.
- 진행 중(2026-11-18): Phase 0 부트스트랩. 구현·Linux 검증·macOS CI 빌드 완료, 사용자 Mac 실행 확인이 남았다.
- 탐지 기능은 아직 없다. 앱은 빈 목록만 보여준다.

상세 계약을 바꾸면 요구사항·설계·단계·수동 기준을 같은 작업에서 갱신한다.
미구현 명령이나 테스트를 이미 쓸 수 있는 것으로 안내하지 않는다.

## 공통 검증 명령

| 명령 | 내용 | 실행 위치 |
| --- | --- | --- |
| `npm run check` | `cargo fmt --check`, 코어·수집기 `cargo clippy -D warnings`, 문서 링크·앵커·ID 검사 | 이 개발 환경, CI(Linux) |
| `npm run check:app` | 셸 크레이트 `cargo clippy -D warnings` | macOS CI. Linux는 WebKitGTK 개발 패키지가 있어야 한다 |
| `npm test` | `cargo test -p portwatch-core -p portwatch-collect` (합성 자료 테스트) | 이 개발 환경, CI(Linux) |
| `npm run test:ui` | 가짜 스냅샷으로 화면 테스트 (Playwright) | Phase 2에서 만든다 |
| `npm run build:mac` | `tauri build --bundles app`으로 .app 생성 | macOS CI(수동 실행), 사용자 Mac |

## Phase 0 — 프로젝트 부트스트랩: 구현 완료, 사용자 Mac 확인 대기

Refs: RQ-07, RQ-10, RQ-11, DC-01, DC-03, DC-05, DC-06, DC-10, DC-19

진입 기준: 계획 문서 4종이 `main`에 있다. DC-03·DC-19를 사용자 Mac에서 확인했다
(2026-11-18: macOS 15.6.1, `arm64`).

- [x] Cargo workspace와 `crates/portwatch-core`(스냅샷 계약 타입과 직렬화 테스트), `crates/portwatch-collect`(빈 스냅샷과 `portwatch-probe`).
- [x] `src-tauri`: 트레이 아이콘(template), 아이콘 아래 팝오버 창, 포커스를 잃으면 닫기, 오른쪽 클릭 종료 메뉴,
  `LSUIElement`와 Accessory 정책(Dock 아이콘 없음), ad-hoc 서명, 최소 macOS 15.0.
- [x] `ui/`: 빈 목록 화면, 경고 줄, 새로 고침·종료 버튼, 밝은·어두운 모드.
- [x] 루트 `package.json`: Tauri CLI 2.12.4, `check`·`check:app`·`test`·`build:mac`·`icons` 스크립트.
- [x] 문서 검사 스크립트(`scripts/check-docs.mjs`): 상대 링크·앵커·ID 참조.
- [x] 아이콘 생성 스크립트(`scripts/make-icons.mjs`)와 생성한 아이콘.
- [x] GitHub Actions(`ci.yml`): Linux에서 check·test(main 푸시·PR), macOS 빌드는 수동 실행만(DC-10). 빌드 결과를 artifact로 올린다.
- [x] 사용한 Tauri·플러그인 버전, 최소 macOS(DC-03), 대상 CPU(DC-19)를 DESIGN.md에 기록했다.
- [x] README에 개발 환경 준비와 서명 없는 빌드를 실행하는 방법을 썼다.
- [x] AGENTS.md의 소스 규칙을 실제 구조에 맞췄다.
- [x] macOS CI 빌드 성공 기록.
- [ ] 사용자 Mac 실행 확인(종료 기준).

기준 검증: 이 개발 환경과 CI에서 `npm run check`·`npm test` 통과, macOS CI 빌드 성공.
종료 기준: 사용자 Mac에서 CI artifact의 앱을 실행하면 메뉴바에 아이콘이 생기고 Dock 아이콘은 없다(M-05 일부).

검증 기록(2026-11-18, 이 개발 환경 Linux, Rust 1.97.0, Node 22.23.5):

- `npm run check` 통과: fmt, 코어·수집기 clippy 경고 0, 문서 6개·ID 61개.
- `npm test` 통과: 코어 2개, 수집기 2개.
- `npm run check:app` 통과: WebKitGTK 개발 패키지를 설치해 셸 크레이트를 Linux로 컴파일했다.
  macOS 전용 코드(`cfg(target_os = "macos")`)는 여기서 컴파일되지 않으므로 macOS CI가 확인한다.
- 화면: Chromium에서 가짜 스냅샷으로 밝은·어두운 모드를 띄워 빈 목록 문구·경고 줄·버튼이 오류 없이 그려지는 것을 봤다.
  이 확인은 자동 테스트로 남기지 않았다(화면 테스트는 Phase 2).

CI 기록(2026-11-18, 커밋 `cdab5cf`):

- push run 44276583474: Linux 작업 성공(check·test, 약 35초). macOS 작업은 설계대로 건너뜀.
- 수동 run 44276588647: Linux 성공, macOS(`macos-15`) 성공. macOS 작업 전체 약 1분 45초
  (`check:app` 35초, `build:mac` 61초, 캐시 없음).
- 빌드 결과 확인: `Signature=adhoc`, `Format=app bundle with Mach-O thin (arm64)`, `LSUIElement`=true,
  `LSMinimumSystemVersion`=15.0. 앱 10.35 MiB, zip artifact 3.08 MB(14일 보관).
- 아직 확인하지 않은 것: 실제 Mac에서의 실행·메뉴바 표시·Dock 숨김·팝오버 위치(종료 기준, 사용자 확인).

Files:
- `.github/workflows/ci.yml` (added)
- `.gitignore` (added)
- `AGENTS.md` (modified)
- `Cargo.lock` (added)
- `Cargo.toml` (added)
- `README.md` (modified)
- `crates/portwatch-collect/Cargo.toml` (added)
- `crates/portwatch-collect/src/bin/portwatch-probe.rs` (added)
- `crates/portwatch-collect/src/lib.rs` (added)
- `crates/portwatch-core/Cargo.toml` (added)
- `crates/portwatch-core/src/lib.rs` (added)
- `crates/portwatch-core/src/snapshot.rs` (added)
- `docs/DESIGN.md` (modified)
- `docs/IMPLEMENT.md` (modified)
- `docs/REQUIREMENTS.md` (modified)
- `package-lock.json` (added)
- `package.json` (added)
- `scripts/check-docs.mjs` (added)
- `scripts/make-icons.mjs` (added)
- `src-tauri/Cargo.toml` (added)
- `src-tauri/Info.plist` (added)
- `src-tauri/build.rs` (added)
- `src-tauri/capabilities/default.json` (added)
- `src-tauri/icons/128x128.png` (added)
- `src-tauri/icons/128x128@2x.png` (added)
- `src-tauri/icons/32x32.png` (added)
- `src-tauri/icons/app-icon.png` (added)
- `src-tauri/icons/icon.icns` (added)
- `src-tauri/icons/icon.png` (added)
- `src-tauri/icons/tray-template.png` (added)
- `src-tauri/src/main.rs` (added)
- `src-tauri/tauri.conf.json` (added)
- `ui/index.html` (added)
- `ui/main.js` (added)
- `ui/style.css` (added)

## Phase 1 — 탐지 코어 (읽기 전용): 대기

Refs: RQ-02, RQ-03, RQ-04, PR-03, PR-04, DC-07, DC-08, DC-09, DC-11, DC-12, M-01, M-02, M-03, M-04

진입 기준: Phase 0 완료.

- [ ] `scripts/collect-mac-sample.sh`: 수집 명령의 원문을 모으고 사용자 이름·홈 경로·호스트 이름을 바꿔 출력한다.
  사용자가 검토한 뒤 공유하며, 저장소에는 이를 바탕으로 만든 합성 자료만 넣는다.
- [ ] 사용자 Mac 표본으로 `ps`의 pgid·lstart 형식과 `launchctl print gui/<uid>` 형식을 확인한다(DC-11).
- [ ] 파서: netstat(헤더 기준 열 찾기), ps, `launchctl print` 목록·상세, lsof 작업 폴더.
- [ ] 판별기: 설계의 판별 순서 1~9, listener 묶기, 분류(DC-12), 프로젝트 이름.
- [ ] 스냅샷 계약 타입과 JSON 직렬화.
- [ ] 수집기와 `portwatch-probe`: 명령을 절대 경로·`LC_ALL=C`로 실행하고 스냅샷 JSON을 출력한다. 실패한 명령은 warnings로 남긴다.
- [ ] 합성 자료 F-01~F-06(수동 검증 문서 참고)과 판별 테스트.

기준 검증: `npm test`의 파서·판별 테스트. F-01(root brew nginx: master·worker)이 `launchd-brew`·system·`nginx`·root로 판별된다.
종료 기준: 사용자 Mac에서 `portwatch-probe` 결과가 M-01~M-04의 탐지 기준을 만족한다. DC-11·DC-12를 결과와 함께 닫는다.

## Phase 2 — 메뉴바 목록 화면: 대기

Refs: RQ-02, RQ-07, PR-03, PR-04, PR-06, PR-07, PR-08, DC-16, DC-20, M-03, M-04, M-05

진입 기준: Phase 1 완료.

- [ ] 트레이 아이콘에 개발 서버·서비스 수 표시.
- [ ] 팝오버 목록: 분류별 묶음, 접힌 분류, 행 상세(프로세스 트리·명령줄·작업 폴더·시작 시각).
- [ ] 열기·URL 복사.
- [ ] 갱신 주기(DC-16): 목록을 연 동안과 닫힌 동안을 나눈다. 수집 실패 시 마지막 정상 스냅샷과 경고를 함께 보여준다.
- [ ] 설정: 로그인 시 자동 실행, 갱신 주기, 숨긴 항목, "개발 서버로 고정", 보안 프로그램 목록.
- [ ] 갱신 중 사용자가 펼친 행·스크롤 위치를 유지한다.

기준 검증: `npm run test:ui`(가짜 스냅샷으로 묶음·접기·상세·갱신 유지). 수집 시간·CPU 측정값을 DC-16에 기록.
종료 기준: 사용자 Mac에서 M-03~M-05 통과.

## Phase 3 — 멈추기: 대기

Refs: RQ-06, PR-01, PR-02, DC-13, DC-14, DC-17, M-06, M-07, M-08, M-09, M-10, M-11

진입 기준: Phase 2 완료.

- [ ] 코어: 관리 주체별 멈추기 계획, 보호 항목 판정, 명령 틀에 넣을 값 검증.
- [ ] 실행기: 프로세스 그룹 신호, `brew services stop`, `launchctl bootout`, `docker stop`, `pm2 stop`, 관리자 권한 실행.
- [ ] 실행 직전 PID·시작 시각·명령줄 재확인, 실행 뒤 최대 5초 재수집과 결과 판정(성공·다시 실행됨·아직 실행 중).
- [ ] 화면: 확인 창(대상·관리 주체·명령·권한), 진행 표시, 결과, 강제 종료 제안.
- [ ] 사용자 Mac에서 DC-14(Docker 형식)·DC-17(root brew 서비스 멈추기 명령)을 확인하고 닫는다.

기준 검증: 계획·보호 판정·값 검증(따옴표·세미콜론 등 주입 시도 거부) 단위 테스트.
이 개발 환경에서 실제 자식 프로세스 그룹을 만들어 SIGTERM·SIGKILL 동작을 확인하는 테스트(신호 동작은 Linux에서도 같다).
종료 기준: 사용자 Mac에서 M-06~M-11 통과.

## Phase 4 — 도메인·프록시·인증서: 대기

Refs: RQ-05, PR-05, DC-15, M-12

진입 기준: Phase 3 완료.

- [ ] hosts 파서: 루프백 도메인만, `localhost`·주석 제외.
- [ ] nginx 설정 읽기(DC-15)와 파서: `include`, `server`, `listen`, `server_name`, `proxy_pass`, `upstream`.
- [ ] 도메인 연결: nginx 항목과 upstream 항목에 붙이기, 직접 80/443을 연 서버.
- [ ] TLS 확인: SNI로 인증서만 읽고 주체·SAN·발급자·만료일·경고 표시.
- [ ] 화면: 행에 도메인 표시, 상세에 연결 경로와 인증서.

기준 검증: hosts·nginx 합성 자료 F-07 파서 테스트. 이 개발 환경에서 자체 서명 인증서로 띄운 TLS 서버를 읽는 테스트.
종료 기준: 사용자 Mac에서 M-12 통과.

## Phase 5 — 배포: 대기

Refs: RQ-08, RQ-09, DC-02, DC-18, DC-19, DC-21, M-13, M-14

진입 기준: Phase 4 완료. DC-18(배포 위치)을 사용자가 정했다.

- [ ] 버전 태그를 푸시하면 macOS CI가 ad-hoc 서명한 앱을 빌드해 zip으로 올리는 workflow.
- [ ] `superlucky84/homebrew-tap` 저장소와 cask(사용자가 저장소를 만든다).
- [ ] Gatekeeper 처리(DC-21)와 README 설치 안내.
- [ ] 업데이트 확인 방법(brew upgrade) 안내.

기준 검증: workflow를 수동 실행으로 리허설하고 산출물의 서명 상태(`codesign -dv`)를 기록한다.
종료 기준: 사용자 Mac에서 `brew install --cask`로 설치·실행·업데이트가 된다(M-13, M-14).

## Phase 6 — Test Hardening: 대기

Refs: RQ-02, RQ-03, RQ-04, RQ-05, RQ-06, DC-12, DC-16

진입 기준: Phase 5 완료.

- [ ] IPv6 전용·특정 주소에만 연 포트, 같은 포트를 여러 프로세스가 연 경우.
- [ ] 수집 중 프로세스가 끝나는 경합, 멈추기 직전 PID 재사용.
- [ ] 긴 명령줄·공백·한글 경로·이상한 프로세스 이름.
- [ ] 명령 없음(docker·brew 미설치), 권한 오류, 출력 형식이 예상과 다른 경우의 warnings.
- [ ] Intel 경로(`/usr/local`)의 brew 서비스 판별.
- [ ] 포트 수백 개 규모의 수집·판별 시간 측정과 회귀 기준.

기준 검증: 위 항목의 자동 테스트 추가와 `npm run check`·`npm test`·`npm run test:ui` 통과.
종료 기준: 측정값과 남은 한계를 이 문서에 기록한다.

## Phase 7 — Integration Test: 대기

Refs: RQ-01, RQ-02, RQ-03, RQ-04, RQ-05, RQ-06, RQ-07, RQ-08, RQ-09, M-01, M-02, M-03, M-04, M-05, M-06, M-07, M-08, M-09, M-10, M-11, M-12, M-13, M-14

진입 기준: Phase 6 완료.

- [ ] brew로 설치한 앱으로 사용자 Mac에서 M-01~M-14 전체를 실행한다.
- [ ] 결과를 수동 검증 문서에 기록하고 실패 항목은 해당 단계로 되돌린다.

기준 검증: 자동 검증 전체 통과.
종료 기준: M-01~M-14가 통과하거나, 남은 항목의 사유와 사용자 승인이 기록된다.

## 현재 인계

이 프로젝트의 현재 인계는 이 블록 하나다. 이전 내용은 git 이력에 남는다.

- 현재 작업(2026-11-18): Phase 0 구현, Linux 검증, macOS CI 빌드(run 44276588647) 완료. 메뉴바 아이콘과 빈 팝오버 창이 있고 탐지 기능은 없다.
- 확인된 사실: 사용자 Mac에서 sudo 없이 root nginx를 netstat·ps·launchctl로 판별할 수 있었다(REQUIREMENTS.md).
  사용자 Mac은 macOS 15.6.1, arm64다.
- 사용자 결정: macOS 전용, 메뉴바 앱·brew 설치·Spotlight 실행, 문서 기반 개발과 `main`, Tauri, Apple 서명 없이 시작.
- 다음: 사용자가 artifact 앱을 Mac에서 실행해 Phase 0 종료 기준을 확인한다. 그다음 Phase 1(탐지 코어)을 시작한다.
- 후속 후보: `upload-artifact`의 `archive: false`로 zip이 이중으로 압축되지 않게 하는 것. 다음 macOS 빌드 때 함께 확인한다.
- 차단: Phase 5 전에 DC-18(비공개 저장소의 배포 위치)을 사용자가 정해야 한다. Phase 1~4에는 차단 없음.
- 커밋: 계획 `4384fd2`, 그리고 이 인계를 포함한 Phase 0 커밋(`main`).
