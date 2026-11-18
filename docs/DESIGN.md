# portwatch 설계

문서 기준: 2026-11-18.
[요구사항](./REQUIREMENTS.md) · [구현 계획](./IMPLEMENT.md) · [수동 검증](./MANUAL_TEST_CHECKLIST.md)

구현된 코드는 아직 없다. 이 문서의 구조·계약·정책은 모두 설계 제안이다.
사용자 Mac에서 확인한 사실과 사용자가 결정한 내용은 그렇다고 표시한다.
기술 구성은 Tauri로 정했다(DC-01, 2026-11-18 사용자 결정).

## 구조와 책임

```text
메뉴바 셸 ── 아이콘·목록 화면·확인 창·갱신 주기
  │   (스냅샷을 받아 그리기만 한다)
  │
  ├── 수집기 ── 정해진 명령을 실행하고 원문 텍스트를 넘긴다 (macOS 의존)
  │
  ├── 코어 (순수 함수, OS 호출 없음)
  │     ├── 파서 ── netstat·ps·launchctl·lsof·docker·hosts·nginx 원문 → 구조
  │     ├── 판별기 ── 포트 → 프로세스 → 관리 주체 → 분류·프로젝트
  │     ├── 도메인 연결 ── hosts·프록시 설정 → 항목에 도메인 붙이기
  │     └── 멈추기 계획 ── 항목 → 실행할 명령과 필요한 권한
  │
  └── 실행기 ── 계획 실행(일반/관리자 권한)·멈춘 뒤 재확인·TLS 확인 (macOS 의존)
```

- 코어는 텍스트를 받아 값을 돌려주는 순수 함수로만 만든다. 그래서 macOS 없이도 합성 출력(fixture)으로 테스트한다.
- 명령 실행, 권한 상승, 화면만 macOS에 의존한다. 이 부분은 사용자 Mac과 macOS CI에서 확인한다.
- 화면은 코어가 만든 스냅샷만 사용한다. 화면에서 직접 명령을 실행하지 않는다.

### Tauri 구성 (제안)

| 위치 | 그림의 역할 | 내용 |
| --- | --- | --- |
| `crates/portwatch-core/` | 코어 | Rust 라이브러리. 의존성은 직렬화 등 순수 계산용만 둔다. macOS 없이 `cargo test`로 검증한다. |
| `crates/portwatch-collect/` | 수집기·실행기 | 명령 실행, 신호 보내기, 관리자 권한 실행, TLS 확인. 진단용 실행 파일 `portwatch-probe`를 함께 둔다. |
| `src-tauri/` | 메뉴바 셸 | 트레이 아이콘, 팝오버 창, 갱신 주기, 화면에 노출하는 고정 명령. |
| `ui/` | 화면 | HTML/CSS/JS. 구현 방식은 DC-20. |

- 루트에 Cargo workspace를 두고 세 크레이트를 묶는다. Tauri CLI와 화면 테스트 도구는 루트 `package.json`의 개발 의존성으로 둔다.
- 메뉴바: Tauri 트레이 아이콘을 누르면 아이콘 아래에 팝오버 창을 띄운다. 창은 포커스를 잃으면 닫는다.
- Dock 아이콘 없음: macOS 활성화 정책을 Accessory로 둔다.
- 로그인 시 자동 실행, 브라우저 열기는 Tauri 공식 플러그인을 쓴다. 정확한 플러그인과 버전은 Phase 0에서 기록한다.
- 화면에 노출하는 명령은 스냅샷 받기, 항목 멈추기, URL 열기·복사, 설정 읽기·쓰기로 고정한다.
  임의 명령을 실행하는 셸 플러그인은 넣지 않는다. 화면이 잘못되거나 변조돼도 정해진 동작 밖으로 나가지 않게 하기 위해서다.
- `portwatch-probe`는 화면 없이 스냅샷을 JSON으로 출력한다. 화면을 만들기 전에 사용자 Mac에서 탐지 결과를 확인하는 데 쓴다.

## 스냅샷 계약

수집 한 번의 결과를 스냅샷 하나로 만든다. 코어와 화면 사이의 계약이다.

```text
Snapshot
  takenAt        수집 시각
  entries[]      Entry 목록
  warnings[]     실패한 명령·읽지 못한 정보 (예: "docker 없음")

Entry            포트를 연 서버 하나 (프로세스 트리의 최상위 기준)
  id             관리 주체 + 최상위 프로세스 PID + 시작 시각으로 만든 값
  title          프로젝트 이름·서비스 이름·컨테이너 이름·실행 파일 이름 중 하나
  category       dev-server | service | dev-tool | app | system
  manager        Manager
  listeners[]    { port, address, family, pid }
  processes[]    { pid, ppid, user, startedAt, command }   최상위부터
  project        { name, path } 또는 없음
  domains[]      { name, via, port, tls }   via: hosts | nginx | direct
  protected      멈추기를 막는지와 그 이유
  stopPlan       멈추기 계획 요약 (권한 필요 여부 포함)

Manager
  kind           launchd-brew | launchd-other | docker | pm2 | terminal | detached | app | launchd-socket | unknown
  domain         system | user (launchd일 때)
  label          launchd 라벨 (예: sh.brew.nginx)
  formula        brew 포뮬러 이름 (예: nginx)
  container      Docker 컨테이너 이름
  appName        앱에서 실행한 경우의 앱 이름
  runsAsRoot     최상위 프로세스가 root인지
```

수집 중 일부 명령이 실패하면 그 정보만 비우고 `warnings`에 남긴다. 직전 스냅샷의 항목을 지어내지 않는다.

## 수집

모든 명령은 절대 경로로 실행하고 `LC_ALL=C`를 준다. Spotlight로 실행한 앱은 셸의 PATH를 물려받지 않기 때문이다.
brew는 `/opt/homebrew/bin/brew`와 `/usr/local/bin/brew`를 차례로 찾는다. 인수는 배열로 넘기고 셸을 거치지 않는다.

| 명령 (sudo 없음) | 얻는 정보 | 근거 |
| --- | --- | --- |
| `/usr/sbin/netstat -anv -p tcp` | 연결을 기다리는 포트와 PID. root·다른 사용자 포함 | 확인된 사실(2026-11-18) |
| `/bin/ps -axo pid=,ppid=,pgid=,user=,lstart=,command=` | 부모·프로세스 그룹·사용자·시작 시각·명령줄 | 명령줄 읽기는 확인됨. pgid·lstart 형식은 Phase 1에서 확인 |
| `/bin/launchctl print system` | root 서비스의 PID → 라벨 표 | 확인된 사실 |
| `/bin/launchctl print gui/<내 uid>` | 사용자 서비스의 PID → 라벨 표 | 형식 확인 필요(DC-11) |
| `/bin/launchctl print <도메인>/<라벨>` | plist 경로·실행 파일·인수·작업 폴더 | 확인된 사실. 서비스 항목에만 실행 |
| `/usr/sbin/lsof -a -p <PID 목록> -d cwd -Fn` | 내 프로세스의 작업 폴더 | 자기 프로세스만 보인다는 사실 확인 |
| `docker ps --format '{{json .}}'` | 컨테이너 이름과 포트 연결 | Docker가 있을 때만. 형식은 DC-14 |
| `/etc/hosts` 읽기 | 루프백을 가리키는 도메인 | Phase 4 |
| nginx 설정 읽기 | server_name·listen·proxy_pass | 방식은 DC-15 |

### netstat 읽기

- 헤더 행에서 열 이름으로 위치를 찾는다. 고정 위치를 쓰지 않는다(macOS 버전마다 다르다).
- `Local Address`·`Foreign Address`처럼 띄어쓰기가 있는 열 이름은 한 열로 합친다.
- `LISTEN` 행만 쓴다. 주소의 마지막 `.` 뒤가 포트다(`127.0.0.1.53733`, `*.8080`).
- `tcp4`·`tcp6`·`tcp46`을 모두 읽고, 같은 PID·포트는 한 listener로 합친다.
- 주소가 `127.0.0.1`·`::1`이면 "이 Mac에서만", `*`이면 "네트워크에서 접근 가능"으로 표시한다.
- netstat의 PID는 소켓을 마지막으로 다룬 프로세스일 수 있다(nginx worker 등). 그래서 항상 부모를 따라 올라가 최상위를 찾는다.

## 판별 순서

listener마다 아래 순서로 관리 주체를 정한다. 먼저 맞는 규칙을 쓴다.

1. PID가 1이면 `launchd-socket`이다. launchd가 직접 연 포트이며 항상 보호한다.
2. `ps` 정보로 부모를 따라 올라가 조상 목록을 만든다. PID 1이나 셸에 닿으면 멈춘다.
3. 조상 중 하나(자기 포함)가 launchd PID 표(system·user)에 있으면 launchd 서비스다.
   그 라벨의 실행 파일이 brew 경로(`/opt/homebrew`, `/usr/local`) 아래면 `launchd-brew`, 아니면 `launchd-other`다.
   포뮬러 이름은 실행 파일 경로의 `opt/<이름>/` 또는 `Cellar/<이름>/`에서 얻는다.
4. listener가 Docker의 포트 전달 프로세스이면 `docker ps`의 포트 연결로 컨테이너를 찾는다(DC-14).
5. 조상에 pm2 데몬(`PM2 … God Daemon`)이 있으면 `pm2`다.
6. 조상에 터미널 셸이 있으면 `terminal`이다. 셸 바로 아래 프로세스를 최상위로 삼는다.
7. 조상이 `.app` 번들 안의 프로세스이면 `app`이다. 그 앱 바로 아래 프로세스를 최상위로 삼는다(예: 에디터에서 띄운 서버).
8. 부모가 PID 1인데 launchd 표에 없으면 `detached`다(`nohup`·`&`·`disown`으로 띄운 경우).
9. 나머지는 `unknown`이다.

라벨 이름 규칙(`homebrew.mxcl.*`, `sh.brew.*`)이나 `ps` 명령줄 글자로 서비스를 맞추지 않는다(DC-09).
같은 최상위 프로세스에 속한 listener는 한 Entry로 묶는다.

### 분류

분류 규칙은 설계 제안이며 Phase 1에서 합성 자료로 확정한다(DC-12).

| 분류 | 규칙 (제안) | 기본 화면 |
| --- | --- | --- |
| system | PID 1, 실행 파일이 `/System`·`/usr/libexec`·`/usr/sbin`·`/sbin` 아래, 보안 프로그램 목록에 있음 | 접힘, 멈추기 금지 |
| service | `launchd-brew`, 보호되지 않는 `launchd-other` | 펼침 |
| dev-server | `terminal`·`detached`·`pm2`·`docker`·`app`(에디터) 관리이고, 루프백 임시 포트(49152~65535)만 열지 않음 | 펼침 |
| dev-tool | 사용자 프로세스가 루프백 임시 포트만 엶(도구 사이 통신으로 추정) | 접힘 |
| app | `.app` 번들 프로세스 자체가 포트를 엶 | 접힘 |

보안 프로그램 목록은 기본값을 두고 사용자가 고칠 수 있게 한다. 사용자는 항목을 "개발 서버로 고정"하거나 숨길 수 있다.

### 프로젝트 이름

`lsof`로 얻은 작업 폴더에서 위로 올라가며 `package.json`·`.git`·`pyproject.toml`·`Cargo.toml`·`go.mod`·`pom.xml`·`build.gradle`을 찾는다.
`package.json`의 `name`이 있으면 그 이름을, 없으면 폴더 이름을 쓴다. 홈 폴더에 닿으면 멈춘다.
root 프로세스는 작업 폴더를 읽지 못하므로 launchd 정보의 작업 폴더를 쓴다.

## 멈추기

### 관리 주체별 계획

| 관리 주체 | 명령 (제안) | 권한 | 비고 |
| --- | --- | --- | --- |
| `terminal`·`detached`·`app` | 최상위 프로세스 그룹에 SIGTERM, 응답이 없으면 사용자 확인 후 SIGKILL | 일반 | nodemon 같은 감시 도구까지 함께 멈춘다 |
| `launchd-brew` (user) | `brew services stop <포뮬러>` | 일반 | kill 하면 KeepAlive가 다시 띄울 수 있다 |
| `launchd-brew` (system) | 관리자 권한으로 `brew services stop <포뮬러>` | 관리자 | 실제 명령 형태는 DC-17 |
| `launchd-other` | `launchctl bootout <도메인>/<라벨>` | 도메인에 따라 | 허용 범위는 DC-13 |
| `docker` | `docker stop <컨테이너>` | 일반 | DC-14 |
| `pm2` | `pm2 stop <이름>` | 일반 | pm2 경로 찾기는 Phase 3 |
| `launchd-socket`·system·보호 항목 | 없음 | — | 버튼을 보여주지 않는다 |
| `unknown` | 최상위 PID에 SIGTERM | 일반 | 결과를 그대로 보고한다 |

### 흐름

1. 사용자가 멈추기를 누르면 확인 창에 대상·관리 주체·실행할 명령·필요한 권한을 보여준다.
2. 실행 직전에 최상위 프로세스의 PID·시작 시각·명령줄이 스냅샷과 같은지 다시 확인한다. 다르면 멈추지 않는다(PID 재사용 방지).
3. 계획을 실행한다.
4. 최대 5초 동안 다시 수집해 결과를 보고한다.
   - 포트가 비고 프로세스가 없으면 성공이다.
   - 같은 관리 주체로 새 프로세스가 떴으면 "다시 실행됨: <관리 주체>"로 알린다.
   - 프로세스가 남아 있으면 "아직 실행 중"으로 알리고, 가능한 경우 강제 종료를 제안한다.

### 관리자 권한

- 앱은 일반 사용자 권한으로만 실행한다. 관리자 권한은 확인한 명령 하나에만 쓴다.
- macOS 관리자 비밀번호 창(`osascript`의 `do shell script … with administrator privileges`)을 쓴다.
- 명령 문자열은 고정 틀에 검증한 값만 넣어 만든다. 라벨은 `^[A-Za-z0-9._-]+$`, 포뮬러는 `^[a-z0-9@+._-]+$`, PID는 숫자만 허용한다.
- 사용자가 비밀번호 창을 취소하면 아무것도 바꾸지 않고 "취소됨"으로 표시한다.
- 권한 상승이 실패하면 같은 명령을 복사해서 터미널에서 실행할 수 있게 보여준다.

### 보호 항목

다음은 멈추기 버튼을 보여주지 않는다: PID 1, `launchd-socket`, system 분류, 보안 프로그램 목록,
portwatch 자신, root·현재 사용자가 아닌 다른 사용자의 프로세스(서비스로 판별된 root 서비스는 예외).

## 도메인 연결

1. `/etc/hosts`에서 `127.0.0.0/8`·`::1`을 가리키는 도메인을 모은다. `localhost`와 주석은 뺀다.
2. 프록시 설정을 읽는다. 처음에는 nginx만 지원한다(Caddy 등은 이후 범위).
   server 블록의 `listen`·`server_name`·`proxy_pass`(upstream 블록 포함)를 읽어
   "도메인 → nginx listener → upstream 포트"를 만든다. 읽기 방식은 DC-15.
3. 도메인을 항목에 붙인다. nginx 항목에는 `via: nginx`로, upstream 포트의 항목에는 같은 도메인을 "nginx 경유"로 붙인다.
   개발 서버가 직접 80/443을 열고 있으면 `via: direct`로 붙인다.
4. TLS 포트이면 `127.0.0.1:<포트>`에 SNI로 도메인을 주고 인증서만 읽는다. 데이터는 보내지 않는다.
   주체·SAN·발급자(mkcert 여부)·만료일을 보여주고, 만료·14일 안 만료·SAN 불일치를 경고한다.

설정 파일과 hosts 파일은 읽기만 한다.

## 화면 (제안)

```text
[메뉴바 아이콘 + 개발 서버·서비스 수]

portwatch                              3초 전 갱신
개발 서버 (2)
  :3000  shop-web   npm run dev      터미널      [열기] [멈추기]
  :8082  api        node server.js   터미널      [열기] [멈추기]
서비스 (1)
  :8080  nginx  root  brew 서비스  local.example.test   [멈추기]
▸ 개발 도구 (4)   ▸ 앱 (1)   ▸ 시스템·보안 (9)
```

- 행을 펼치면 프로세스 트리·명령줄·작업 폴더·시작 시각·도메인·인증서가 보인다.
- 열기는 `http(s)://<도메인 또는 localhost>:<포트>`를 기본 브라우저로 연다. URL 복사도 둔다.
- Dock 아이콘은 없다. 설정에 로그인 시 자동 실행·갱신 주기·보안 프로그램 목록·숨긴 항목을 둔다.
- 화면 문자열은 한국어로 시작한다.

## 배포

- 앱 파일은 GitHub Releases에 올리고, 설치 정보는 `superlucky84/homebrew-tap` 저장소의 cask로 둔다.
  설치 명령은 `brew install --cask superlucky84/tap/portwatch`다.
- `/Applications`에 설치되므로 Spotlight 검색은 따로 만들지 않는다.
- 저장소가 비공개라서 Homebrew가 Releases 파일을 인증 없이 받을 수 없다. 배포 위치는 DC-18에서 정한다.
- Apple 서명·공증 없이 시작한다(DC-02). 대신 ad-hoc 서명을 한다. 서명이 전혀 없는 Apple Silicon 앱은
  내려받은 뒤 "손상된 앱"으로 표시될 수 있기 때문이다(Phase 5에서 확인).
- 내려받은 앱에는 macOS의 격리 표시가 붙어 첫 실행을 Gatekeeper가 막는다. macOS 15부터는 설정의
  "개인정보 보호 및 보안 > 그래도 열기"로 허용하며, 새 버전마다 다시 허용해야 할 수 있다.
  처리 방식(허용 절차 안내 또는 cask 설치 단계에서 격리 표시 제거)은 DC-21에서 정한다.
- Mac에서 직접 빌드해 설치한 앱에는 격리 표시가 붙지 않아 이 절차가 필요 없다.
- 회사 보안 에이전트가 서명 없는 앱을 막을 수 있다. M-14에서 확인한다.

## 안전 정책

- 탐지는 sudo 없이 한다. 앱 전체를 root로 실행하지 않는다.
- 보호 항목은 멈추지 않는다. 멈추기는 항상 사용자 확인을 거친다.
- 외부 네트워크로 아무것도 보내지 않는다. 네트워크 접속은 루프백 TLS 확인뿐이다.
- 테스트 자료는 합성 자료만 쓴다. 사용자 이름·홈 경로·사내 정보를 저장소에 넣지 않는다.

## 결정 체크리스트

설계 결정을 해소할 때 근거와 대응 검증을 함께 기록한다.

- [x] DC-01: 기술 구성은 Tauri다. 2026-11-18 사용자 결정("Tauri로 하는게 좋겠네").
  코어는 Rust, 화면은 HTML/CSS/JS, 셸은 Tauri 트레이 아이콘과 팝오버 창이다.
  근거: 코어 테스트를 이 개발 환경(Linux, Rust 설치됨)에서 바로 돌릴 수 있고, 화면이 웹 기술이라 수정과 검토가 익숙하다.
  비교한 대안: Swift(가장 Mac답고 가볍지만 이 개발 환경에 Swift가 없어 모든 빌드·테스트를 Mac에서 해야 함),
  Electron(상주 앱치고 무거움). .app 빌드는 macOS에서 한다. RQ-11. 검증: Phase 0 종료 기준.
- [x] DC-02: Apple 서명·공증 없이 시작한다. 2026-11-18 사용자 결정("일단 애플서명없이").
  ad-hoc 서명만 하고, Gatekeeper 허용 절차를 안내한다(DC-21). 동료에게 배포하기로 하면 Apple Developer Program(연 $99)
  가입과 공증을 다시 검토한다. 공증을 나중에 붙여도 앱 구조는 바뀌지 않는다. RQ-09. 검증: M-14.
- [ ] DC-03: 최소 지원 macOS 버전. TBD — 사용자 Mac 버전 확인 뒤 정한다. 검증: Phase 0, M-13.
- [x] DC-04: macOS 전용으로 시작한다. 2026-11-18 사용자 결정("일단 mac에서만 되면 될 거 같은데"). RQ-01.
- [x] DC-05: 메뉴바 상주 앱, Homebrew cask 설치, Spotlight 실행. 2026-11-18 사용자 선호.
  앞서 제안했던 "Node CLI + 로컬 웹 대시보드"는 이 결정으로 접었다. RQ-07, RQ-08. 검증: M-05, M-13.
- [x] DC-06: docatlas의 문서 기반 개발 방식, `main` 브랜치. 2026-11-18 사용자 결정. RQ-10.
- [x] DC-07: 포트 목록은 `lsof`가 아니라 `netstat -anv`로 만든다. 사용자 Mac에서 root nginx의 8080이
  netstat에는 PID와 함께 보이고 lsof에는 보이지 않았다. lsof는 자기 프로세스의 작업 폴더에만 쓴다. 검증: Phase 1 파서 테스트, M-01.
- [x] DC-08: root 서비스의 PID → 라벨은 `launchctl print system` 한 번으로 얻는다. sudo 없이 `PID 상태 라벨` 행이 나왔다.
  서비스 상세는 `launchctl print system/<라벨>`로 얻는다(plist·실행 파일·작업 폴더·state·pid 확인). 검증: Phase 1, M-01.
- [x] DC-09: 서비스는 이름 규칙이나 명령줄 글자가 아니라 launchd의 PID와 plist 실행 경로로 맞춘다.
  근거: 라벨이 `homebrew.mxcl.*`가 아니라 `sh.brew.nginx`였고, nginx는 명령줄을 바꿔 쓰며,
  sudo 없는 `brew services list`는 실행 중인 root 서비스를 `none`으로 보였다. 검증: Phase 1 판별 테스트.
- [ ] DC-10: CI 구성과 비용. 제안: Linux 작업에서 코어 테스트를 매번 돌리고, macOS 빌드는 main 푸시·태그·수동 실행에서만 돌린다.
  비공개 저장소의 macOS 실행 시간은 유료 한도에 더 크게 잡히므로 Phase 0에서 현재 요금 기준을 확인한다.
- [ ] DC-11: 사용자 서비스 `launchctl print gui/<uid>`의 형식이 system과 같은지. Phase 1에서 사용자 Mac으로 확인한다.
- [ ] DC-12: 분류 규칙(임시 포트 범위, 보안 프로그램 기본 목록, 실행 파일 경로 규칙). Phase 1에서 합성 자료로 확정한다.
- [ ] DC-13: brew가 아닌 launchd 서비스를 멈출 수 있게 할지와 범위. 제안: `/Library`·`~/Library` 아래 사용자가 설치한
  plist만 허용하고 Apple 서비스는 막는다. Phase 3.
- [ ] DC-14: Docker Desktop(또는 OrbStack·colima)에서 포트를 여는 프로세스 형식과 컨테이너 연결 방법. Phase 3에서 사용자 Mac으로 확인한다.
- [ ] DC-15: nginx 설정 읽기 방식. `nginx -T`(전체 설정 출력)와 설정 파일 직접 읽기 중 고른다.
  `nginx -T`가 일반 권한으로 동작하는지(로그 파일 권한 등) 확인한다. Phase 4.
- [ ] DC-16: 갱신 주기. 제안: 목록을 연 동안 3초, 닫힌 동안 15초. 수집 시간과 CPU 사용을 Phase 2에서 측정해 정한다.
- [ ] DC-17: root brew 서비스를 관리자 권한으로 멈추는 실제 명령. 관리자 권한 셸에서 `brew services stop`이 동작하는지,
  아니면 `launchctl bootout system/<라벨>`과 plist 처리를 직접 해야 하는지 Phase 3에서 사용자 Mac으로 확인한다. 검증: M-08.
- [ ] DC-18: 배포 위치. TBD — 사용자 결정 필요. 비공개 저장소의 Releases는 Homebrew가 인증 없이 받을 수 없다.
  - (a) portwatch 저장소를 공개로 바꾼다. 가장 단순하다.
  - (b) 코드는 비공개로 두고 빌드한 앱만 공개 저장소(예: `superlucky84/homebrew-tap`의 Releases)에 올린다.
  - (c) brew 없이 Mac에서 직접 빌드해 `/Applications`에 설치한다. Gatekeeper 절차가 필요 없지만 RQ-08과 다르다.
  Phase 5 전에 정한다. 검증: M-13.
- [ ] DC-19: 대상 CPU. 제안: Apple Silicon(arm64)만 먼저 빌드하고, Intel Mac 사용자에게 배포할 때 universal 빌드를 추가한다.
  사용자 Mac은 `/opt/homebrew`를 쓰므로 Apple Silicon으로 보인다. Phase 0에서 확인한다.
- [ ] DC-20: 화면 구현 방식. 제안: 프레임워크 없이 ES 모듈로 시작한다(docatlas 화면과 같은 방식).
  화면 테스트는 Tauri 호출을 가짜로 바꾸고 Playwright로 이 개발 환경에서 돌린다. Phase 2에서 확정한다.
- [ ] DC-21: Gatekeeper 처리. 허용 절차를 README로 안내할지, cask 설치 단계에서 격리 표시를 지울지 고른다.
  후자는 편하지만 Gatekeeper 확인을 건너뛰므로 회사 보안 정책에 맞는지 사용자가 판단해야 한다. Phase 5. 검증: M-14.
