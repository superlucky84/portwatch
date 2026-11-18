# portwatch 구현 계획

문서 기준: 2026-11-18.
[요구사항](./REQUIREMENTS.md) · [설계](./DESIGN.md) · [수동 검증](./MANUAL_TEST_CHECKLIST.md)

## 현재 상태와 작업 순서

- 완료(2026-11-18): 사용자 Mac에서 sudo 없이 root 서비스를 탐지할 수 있는지 시험했다(요구사항의 확인된 사실).
- 완료(2026-11-18): 기존 도구 검토. 사용자가 LeftOpen을 직접 시험했고 root로 띄운 서비스를 잡지 못했다.
- 완료(2026-11-18): 계획 문서 4종 초안. 사용자 결정 DC-01(Tauri)·DC-02(서명 없이 시작)·DC-04~06을 반영했다.
- 다음: Phase 0 부트스트랩. 시작 전에 DC-03(macOS 버전)·DC-19(CPU)를 사용자 Mac에서 확인한다.
- 구현된 코드는 없다. 아래 명령(`cargo test` 등)은 Phase 0이 끝나야 쓸 수 있다.

상세 계약을 바꾸면 요구사항·설계·단계·수동 기준을 같은 작업에서 갱신한다.
미구현 명령이나 테스트를 이미 쓸 수 있는 것으로 안내하지 않는다.

## 공통 검증 명령 (Phase 0에서 만든다)

| 명령 | 내용 | 실행 위치 |
| --- | --- | --- |
| `npm run check` | `cargo fmt --check`, `cargo clippy -D warnings`, 문서 상대 링크 검사 | 이 개발 환경, CI(Linux) |
| `npm test` | `cargo test -p portwatch-core` (합성 자료 테스트) | 이 개발 환경, CI(Linux) |
| `npm run test:ui` | 가짜 스냅샷으로 화면 테스트 (Playwright) | 이 개발 환경, CI(Linux). Phase 2부터 |
| `npm run build:mac` | `tauri build`로 .app 생성 | macOS CI, 사용자 Mac |

## Phase 0 — 프로젝트 부트스트랩: 대기

Refs: RQ-07, RQ-10, RQ-11, DC-01, DC-03, DC-05, DC-06, DC-10, DC-19

진입 기준: 계획 문서 4종이 `main`에 있다. DC-03·DC-19를 사용자 Mac에서 확인했다
(`sw_vers -productVersion`, `uname -m`).

- [ ] Cargo workspace와 `crates/portwatch-core`(빈 스냅샷 타입과 테스트 하나), `crates/portwatch-collect`(빈 뼈대).
- [ ] `src-tauri`: 트레이 아이콘, 빈 팝오버 창, Accessory 정책(Dock 아이콘 없음).
- [ ] `ui/`: 빈 목록 화면.
- [ ] 루트 `package.json`: Tauri CLI, `check`·`test`·`build:mac` 스크립트.
- [ ] GitHub Actions: Linux에서 check·test, macOS에서 `build:mac`(main 푸시·수동 실행만, DC-10). 빌드 결과를 artifact로 올린다.
- [ ] 사용한 Tauri와 플러그인 버전, 최소 macOS(DC-03), 대상 CPU(DC-19)를 DESIGN.md에 기록한다.
- [ ] README에 개발 환경 준비와 서명 없는 빌드를 실행하는 방법을 쓴다.
- [ ] AGENTS.md의 소스 규칙을 실제 구조에 맞춘다.

기준 검증: 이 개발 환경과 CI에서 `npm run check`·`npm test` 통과, macOS CI 빌드 성공.
종료 기준: 사용자 Mac에서 CI artifact의 앱을 실행하면 메뉴바에 아이콘이 생기고 Dock 아이콘은 없다(M-05 일부).

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

- 현재 작업(2026-11-18): 계획 문서 4종, AGENTS.md, README.md 초안 작성. 구현 코드는 없다.
- 확인된 사실: 사용자 Mac에서 sudo 없이 root nginx를 netstat·ps·launchctl로 판별할 수 있었다(REQUIREMENTS.md).
- 사용자 결정: macOS 전용, 메뉴바 앱·brew 설치·Spotlight 실행, 문서 기반 개발과 `main`, Tauri, Apple 서명 없이 시작.
- 다음: 사용자 Mac에서 `sw_vers -productVersion`·`uname -m`을 확인해 DC-03·DC-19를 정하고 Phase 0을 시작한다.
- 차단: Phase 5 전에 DC-18(비공개 저장소의 배포 위치)을 사용자가 정해야 한다. Phase 0~4에는 차단 없음.
- 커밋: 이 인계를 포함한 첫 커밋(`main`).
