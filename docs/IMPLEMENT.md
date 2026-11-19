# portwatch 구현 계획

문서 기준: 2026-11-18.
[요구사항](./REQUIREMENTS.md) · [설계](./DESIGN.md) · [수동 검증](./MANUAL_TEST_CHECKLIST.md)

## 현재 상태와 작업 순서

- 완료(2026-11-18): 사용자 Mac에서 sudo 없이 root 서비스를 탐지할 수 있는지 시험했다(요구사항의 확인된 사실).
- 완료(2026-11-18): 기존 도구 검토. 사용자가 LeftOpen을 직접 시험했고 root로 띄운 서비스를 잡지 못했다.
- 완료(2026-11-18): 계획 문서 4종 초안. 사용자 결정 DC-01(Tauri)·DC-02(서명 없이 시작)·DC-04~06을 반영했다.
- 완료(2026-11-18): Phase 0 부트스트랩. 사용자 Mac에서 메뉴바 아이콘·Dock 숨김·창 열고 닫기·종료를 확인했다.
- 완료(2026-11-19): Phase 1 탐지 코어. 사용자 Mac에서 root·사용자 brew 서비스, 터미널 서버, 잡음 분류를 probe로 확인했다.
- 다음(2026-11-19): Phase 2 메뉴바 목록 화면.
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

## Phase 0 — 프로젝트 부트스트랩: 완료

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
- [x] 사용자 Mac 실행 확인(종료 기준). 2026-11-18 사용자 확인 "모두 잘됨".

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
- 사용자 Mac(macOS 15.6.1, arm64) 확인(2026-11-18): artifact 앱을 실행해 메뉴바 아이콘 표시, Dock 아이콘 없음,
  왼쪽 클릭으로 아이콘 아래 창 열기와 바깥 클릭으로 닫기, 오른쪽 클릭 메뉴와 창 버튼으로 종료가 모두 됐다("모두 잘됨").
  회사 보안 프로그램이 실행을 막지 않았다. M-05의 메뉴바·Dock 항목에 해당하며, 자동 갱신·한국어 항목은 Phase 2에서 본다.

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

## Phase 1 — 탐지 코어 (읽기 전용): 완료

Refs: RQ-02, RQ-03, RQ-04, PR-03, PR-04, DC-07, DC-08, DC-09, DC-11, DC-12, M-01, M-02, M-03, M-04

진입 기준: Phase 0 완료(2026-11-18).

2026-11-18 계획 변경: 별도 셸 스크립트(`scripts/collect-mac-sample.sh`) 대신 `portwatch-probe --raw`가 사용자 이름·홈 경로를
가린 원문을 출력한다. CI가 빌드한 실행 파일 하나로 탐지 결과와 원문 형식을 함께 확인하고, 가리기 로직도 테스트할 수 있기 때문이다.

- [x] 파서(`parse.rs`): netstat(헤더 기준 열 찾기), ps(`-ww`, 고정 열 순서, `nobody`의 uid `-2` 처리), `launchctl print` 목록·상세, lsof 작업 폴더.
- [x] 판별기(`resolve.rs`): 설계의 판별 순서, listener 묶기, 분류(DC-12 제안), 보호 이유, 제목, 정렬.
  수집 두 단계를 위한 추가 조회 계획(`plan_lookups`)을 둔다.
- [x] 프로젝트 규칙(`project.rs`)과 가리기(`privacy.rs`): 홈 경로·단어 단위 사용자 이름.
- [x] 스냅샷 계약 보강: 프로세스의 pgid·uid·tty.
- [x] 수집기: 명령을 절대 경로·`LC_ALL=C`·빈 환경으로 실행하고 5초 시간 제한을 둔다. 출력을 따로 읽어 파이프가 막히지 않게 한다.
  실패한 명령은 warnings로 남긴다. 명령 실행(`Runner`)과 파일 확인(`Files`)을 바꿔 끼워 합성 자료로 수집 흐름 전체를 테스트한다.
- [x] `portwatch-probe`: 기본은 분류별 표(`report.rs`), `--json`은 스냅샷, `--raw`는 가린 원문(`raw.rs`).
- [x] 합성 자료 F-01~F-04·F-06과 판별·수집 테스트(F-02는 DC-11 확인 전까지 가정한 형식).
- [x] 메뉴바 앱의 `get_snapshot`이 실제 수집을 별도 스레드에서 실행한다. 목록 화면은 Phase 2에서 만든다.
- [x] macOS CI: artifact에 `portwatch-probe`를 함께 넣고(`archive: false`로 이중 압축 없앰), 실행기에서 probe를 한 번 돌린다.
- [x] 사용자 Mac에서 `portwatch-probe`와 `--raw` 결과로 DC-11(사용자 서비스 형식)·ps 형식을 확인하고 합성 자료를 맞춘다.

기준 검증: `npm test`의 파서·판별·수집 흐름 테스트. F-01(root brew nginx: master·worker)이 `launchd-brew`·system·`nginx`·root로 판별된다.
종료 기준: 사용자 Mac에서 `portwatch-probe` 결과가 M-01~M-04의 탐지 기준을 만족한다. DC-11·DC-12를 결과와 함께 닫는다.

검증 기록(2026-11-18, 커밋 `b9d2e44`):

- 이 개발 환경(Linux): `npm run check` 통과, `npm test` 34개 통과(코어 단위 18, 합성 시나리오 7, 수집기 9).
  `npm run check:app` 통과. probe는 Linux에서 "macOS가 아니어서 수집하지 않았습니다." 경고만 낸다.
- CI run 44282667243(수동): Linux 성공, macOS 성공(작업 약 1분 37초). artifact는 앱과 probe를 담은 zip 하나(3.58 MB, 이중 압축 없음).
- macOS 실행기(15.8.2)에서 probe 실행: 항목 1개(launchd가 연 22번, 시스템·보안, 보호)로 맞게 판별했다. `--raw`로 본 형식:
  netstat 헤더는 사용자 Mac과 같다. ps 열은 사용자 이름 칸이 넓게 채워져도 읽힌다.
  `launchctl print system`·`gui/501`의 서비스 줄은 `PID 상태<탭>라벨`이며 공백 구분으로 읽힌다.
  `gui/501`은 `type = login`이지만 `services = { … }` 형식은 system과 같다.
- 실행기에는 개발 서버·brew 서비스가 없어서 분류 규칙(DC-12)과 사용자 brew 서비스(DC-11)의 실제 사례는 사용자 Mac에서 확인한다.

사용자 Mac 1차(2026-11-19, macOS 15.6.1, run 44282667243의 probe, 회사·제조사 이름은 기록하지 않는다):

- 맞음: `sudo brew services`로 띄운 nginx가 "brew 서비스(system) root" 한 항목(master·worker)으로 나왔다(M-01 탐지).
  tmux 안 셸에서 띄운 pnpm 모노레포 개발 서버가 하위 프로세스 9개와 함께 한 항목, 하위 패키지 이름으로 나왔다(M-03 일부).
  launchd가 연 22·4389번, system 도메인 보안 서비스 4개가 시스템·보안으로 접혔다. 사용자 도메인 LaunchAgent 3개를 판별했다(DC-11).
  서비스 목록 상태 칸의 `(pe)` 값도 읽었다.
- 고침(DC-12): 실행 경로에 공백이 있는 보안 에이전트가 서비스로 보였다 → 명령줄 전체·라벨·서비스 실행 경로로 비교.
  기본 목록에 없던 보안 제조사의 에이전트가 개발 도구로 보였다 → 기본 목록에 국내 보안 제조사 이름 추가.
  `/`에서 도는 분리 실행 데몬이 개발 서버로 보였다 → 프로젝트 없는 `detached`는 개발 도구.
  프로젝트 폴더에서 실행한 코딩 도구가 그 프로젝트 이름으로 보였다 → 개발 도구는 실행 파일 이름을 제목으로.
- 위 네 경우를 합성 자료에 넣었다(공백 경로 LaunchAgent, `/`에서 도는 brew 데몬, 프로젝트 안 도구). `npm test` 36개 통과.

사용자 Mac 2차(2026-11-19, 커밋 `ab0d848`, run 44441488154의 probe): 항목 14개. 개발 서버는 모노레포 서버 1개, 서비스는 root nginx 1개만
남았다. 분리 실행 데몬과 코딩 도구 3개(제목은 실행 파일 이름, 프로젝트는 따로 표시)는 개발 도구, 보안 프로그램 6개·Apple 서비스·
launchd 소켓은 시스템·보안으로 접혔다. DC-12를 닫았다. CI run 44441488154는 Linux·macOS 모두 성공(macOS 작업 약 1분 44초).

사용자 Mac 3차(2026-11-19): root nginx를 멈추고 소유자를 되돌린 뒤 `brew services start nginx`(sudo 없음)로 띄웠다. probe에서
nginx가 "brew 서비스(user)", 라벨 `sh.brew.nginx`, `root` 표시 없이 한 항목(master·worker)으로 나왔다(M-02 탐지).
사용자 도메인에서도 brew 라벨이 `sh.brew.*`였다. 그 밖의 분류는 2차와 같았다.

종료 판단(2026-11-19): M-01·M-02·M-04의 탐지 기준을 사용자 Mac probe로 확인했다. M-03은 터미널 서버 1개만 실제로 봤고,
여러 서버·감시 도구 경우는 합성 자료(F-03) 테스트로 확인했으며 실제 확인은 Phase 2 화면 확인과 Phase 7로 넘긴다.
화면 표시(접기·숨기기·자동 갱신)는 Phase 2 범위다.

Files:
- `.github/workflows/ci.yml` (modified)
- `README.md` (modified)
- `crates/portwatch-collect/src/bin/portwatch-probe.rs` (modified)
- `crates/portwatch-collect/src/lib.rs` (modified)
- `crates/portwatch-collect/src/raw.rs` (added)
- `crates/portwatch-collect/src/report.rs` (added)
- `crates/portwatch-collect/src/system.rs` (added)
- `crates/portwatch-core/Cargo.toml` (modified)
- `crates/portwatch-core/src/lib.rs` (modified)
- `crates/portwatch-core/src/parse.rs` (added)
- `crates/portwatch-core/src/privacy.rs` (added)
- `crates/portwatch-core/src/project.rs` (added)
- `crates/portwatch-core/src/resolve.rs` (added)
- `crates/portwatch-core/src/snapshot.rs` (modified)
- `crates/portwatch-core/tests/fixtures/mac-15/launchctl-gui.txt` (added)
- `crates/portwatch-core/tests/fixtures/mac-15/launchctl-system.txt` (added)
- `crates/portwatch-core/tests/fixtures/mac-15/lsof-cwd.txt` (added)
- `crates/portwatch-core/tests/fixtures/mac-15/netstat.txt` (added)
- `crates/portwatch-core/tests/fixtures/mac-15/ps.txt` (added)
- `crates/portwatch-core/tests/fixtures/mac-15/service-gui-homebrew.mxcl.redis.txt` (added)
- `crates/portwatch-core/tests/fixtures/mac-15/service-gui-kr.example.crossex-service.txt` (added)
- `crates/portwatch-core/tests/fixtures/mac-15/service-system-com.apple.Kerberos.kdc.txt` (added)
- `crates/portwatch-core/tests/fixtures/mac-15/service-system-com.vendor.agentd.txt` (added)
- `crates/portwatch-core/tests/fixtures/mac-15/service-system-sh.brew.nginx.txt` (added)
- `crates/portwatch-core/tests/mac_fixtures.rs` (added)
- `docs/DESIGN.md` (modified)
- `docs/IMPLEMENT.md` (modified)
- `docs/MANUAL_TEST_CHECKLIST.md` (modified)
- `src-tauri/src/main.rs` (modified)

## Phase 2 — 메뉴바 목록 화면: 진행 중

Refs: RQ-02, RQ-07, PR-03, PR-04, PR-06, PR-07, PR-08, DC-16, DC-20, M-03, M-04, M-05

진입 기준: Phase 1 완료(2026-11-19). 사용자가 리뷰 없이 바로 진행하기로 했다(2026-11-19).

- [x] 코어: 항목 key(개발 도구는 실행 파일 이름), 숨기기·고정 적용과 아이콘 숫자(`prefs.rs`), 설정 모양·범위 정리(`settings.rs`),
  URL 열기·복사 입력 검증(`actions.rs`), 스냅샷 `collectMs`.
- [x] 수집기: 설정의 보안 프로그램 목록을 받아 쓰고(`collect_snapshot_using`) 수집 시간을 잰다. probe 표 첫 줄에 수집 시간을 쓴다.
- [x] 셸: 갱신 루프(`refresh.rs`, 열린/닫힌 주기, 바로 깨우기), 아이콘 옆 숫자, `snapshot` 이벤트, 설정 파일 읽기·쓰기(`state.rs`,
  임시 파일 후 이름 바꾸기), 설계의 고정 명령 목록(`commands.rs`), 자동 실행 플러그인(셸 안에서만 호출), Esc로 닫기.
- [x] 화면(`ui/main.js`·`ui/view.js`): 분류별 목록과 접기, 행 상세, 열기·URL 복사, 숨기기·고정, 설정 화면, 경고 줄,
  갱신 때 상태 유지(내용이 같으면 다시 그리지 않음). 바깥 문자열은 모두 `textContent`로 넣는다.
- [x] 화면 테스트 `npm run test:ui`(가짜 `window.__TAURI__`, Playwright 1.63.0) 8개와 Linux CI 추가.
- [ ] 사용자 Mac: 수집 시간·CPU 측정으로 DC-16 확정, M-03~M-05 확인.

기준 검증: `npm run check`·`npm test`·`npm run test:ui` 통과, `npm run check:app`(Linux) 통과, macOS CI 빌드 성공.
종료 기준: 사용자 Mac에서 M-03~M-05 통과(목록·접기·숨기기·고정·자동 갱신·아이콘 숫자). DC-16을 측정값과 함께 닫는다.

검증 기록(2026-11-19, 이 개발 환경 Linux):

- `npm run check` 통과, `npm test` 43개 통과(설정 4·선호 1·입력 검증 2개 추가), `npm run check:app` 통과.
- `npm run test:ui` 8개 통과: 기본 펼침·접기, 상세와 글자 처리, 열기·복사·숨기기·고정이 부르는 명령, 갱신 때 상태·스크롤 유지와
  같은 내용 다시 그리지 않음, 경고, 설정 화면, Esc, 셸 밖 안내.
- 380×520 밝은·어두운 모드 스크린샷으로 목록·상세·설정 배치를 봤다(자동 테스트로는 남기지 않음).
- 아직 확인하지 않은 것: 실제 Tauri 셸의 갱신 루프·아이콘 숫자·자동 실행·설정 파일·열기·복사(macOS에서만 확인 가능).

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

- [ ] macOS CI에서 실제 서비스를 띄워 probe로 판별하는 시나리오(예: brew nginx를 사용자·root 서비스로, 백그라운드 서버). 실행기는 sudo에 비밀번호가 없다.
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

- 현재 작업(2026-11-19): Phase 2 메뉴바 목록 화면. 코어·수집기·셸·화면 구현과 Linux 검증(Rust 43개, 화면 8개) 완료.
  macOS CI 빌드와 사용자 Mac 확인(M-03~M-05, DC-16 측정)이 남았다.
- 확인된 사실: 사용자 Mac은 macOS 15.6.1, arm64. sudo 없이 root 서비스까지 판별된다. brew 서비스 라벨은 사용자·root 모두 `sh.brew.*`였다.
- 사용자 결정: macOS 전용, 메뉴바 앱·brew 설치·Spotlight 실행, 문서 기반 개발과 `main`, Tauri, Apple 서명 없이 시작,
  Phase 1 리뷰 없이 Phase 2 진행(2026-11-19).
- 다음: macOS CI(수동 실행) 빌드 → 사용자가 앱을 실행해 M-03~M-05를 보고, 창 아래쪽 수집 시간과 활동 모니터의 CPU를 알려 주면
  DC-16을 닫고 Phase 2를 마친다.
- 차단: Phase 5 전에 DC-18(비공개 저장소의 배포 위치)을 사용자가 정해야 한다. Phase 2~4에는 차단 없음.
- 커밋: 계획 `4384fd2`, Phase 0 `cdab5cf`·`79a25b8`, Phase 1 `b9d2e44`~`964f205`, 그리고 이 인계를 포함한 Phase 2 구현 커밋(`main`).
