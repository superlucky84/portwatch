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
| `crates/portwatch-core/` | 코어 | Rust 라이브러리. 의존성은 직렬화·인증서 해석(`x509-parser`) 등 순수 계산용만 둔다. macOS 없이 `cargo test`로 검증한다. |
| `crates/portwatch-collect/` | 수집기·실행기 | 명령 실행, 신호 보내기, 관리자 권한 실행, TLS 확인. 진단용 실행 파일 `portwatch-probe`를 함께 둔다. |
| `src-tauri/` | 메뉴바 셸 | 트레이 아이콘, 팝오버 창, 갱신 주기, 화면에 노출하는 고정 명령. |
| `ui/` | 화면 | HTML/CSS/JS. 구현 방식은 DC-20. |

- 루트에 Cargo workspace를 두고 세 크레이트를 묶는다. Tauri CLI와 화면 테스트 도구는 루트 `package.json`의 개발 의존성으로 둔다.
- 메뉴바: Tauri 트레이 아이콘을 왼쪽 클릭하면 아이콘 바로 아래(positioner의 `TrayBottomCenter`)에 팝오버 창을 띄운다.
  창은 포커스를 잃으면 닫는다. 아이콘 클릭으로 창이 포커스를 잃은 직후 300ms 안의 같은 클릭은 다시 열지 않는다.
  오른쪽 클릭 메뉴와 창 아래쪽 버튼으로 종료한다. Dock 아이콘이 없어서 종료 수단을 따로 둔다.
- Dock 아이콘 없음: `Info.plist`의 `LSUIElement`와 실행 중 Accessory 활성화 정책을 함께 쓴다.
- 메뉴바 아이콘은 검은색 template 이미지(44px)라 macOS가 밝기·어두운 모드에 맞춰 색을 바꾼다. 아이콘은 `npm run icons`로 다시 그린다.
- 로그인 시 자동 실행, 브라우저 열기는 Tauri 공식 플러그인을 Phase 2에서 추가한다.

Phase 0에서 고정한 버전(2026-11-18, 게시 후 4주 안팎 지난 버전을 골랐다):

| 구성 | 버전 |
| --- | --- |
| `tauri` | 2.12.5 |
| `tauri-build` | 2.7.4 |
| `@tauri-apps/cli` | 2.12.4 |
| `tauri-plugin-positioner` | 2.4.0 |
| `tauri-plugin-autostart` | 2.7.0 (Phase 2) |
| `@playwright/test` | 1.63.0 (Phase 2, 화면 테스트) |
| `x509-parser` | 0.18.1 (Phase 4, 코어의 인증서 해석) |
| `rustls` | 0.23.46 (Phase 4, 수집기의 TLS 확인. 기본 기능을 끄고 `ring`·`std`·`tls12`만 켠다) |
| `rcgen` | 0.14.10 (Phase 4, 테스트에서만 인증서를 만든다) |
| 개발 환경 Rust | 1.97.0 (클라우드 세션), 1.100.0 (사용자 Mac, Phase 3부터) |
- 화면에 노출하는 명령은 "화면과 갱신" 절의 목록으로 고정한다(멈추기는 Phase 3에서 추가).
  임의 명령을 실행하는 셸 플러그인은 넣지 않는다. 화면이 잘못되거나 변조돼도 정해진 동작 밖으로 나가지 않게 하기 위해서다.
  자동 실행 플러그인도 화면에 권한을 주지 않고 셸 명령 안에서만 부른다.
- `portwatch-probe`는 화면 없이 스냅샷을 JSON으로 출력한다. 화면을 만들기 전에 사용자 Mac에서 탐지 결과를 확인하는 데 쓴다.

## 스냅샷 계약

수집 한 번의 결과를 스냅샷 하나로 만든다. 코어와 화면 사이의 계약이다.

Phase 0에서 `crates/portwatch-core/src/snapshot.rs`에 이 모양으로 구현했다. JSON 필드 이름은 camelCase,
분류·관리 주체 값은 kebab-case(`dev-server`, `launchd-brew`)다.

```text
Snapshot
  takenAtMs      수집 시각 (Unix epoch 밀리초)
  collectMs      수집에 걸린 시간 (밀리초). Phase 2에서 추가
  entries[]      Entry 목록
  warnings[]     실패한 명령·읽지 못한 정보 (예: "docker 없음")

Entry            포트를 연 서버 하나 (프로세스 트리의 최상위 기준)
  id             관리 주체 + 최상위 프로세스 PID + 시작 시각으로 만든 값
  key            다시 실행해도 같은 항목 기준 (숨기기·고정에 쓴다). Phase 2에서 추가
  hidden         사용자가 숨긴 항목인지. Phase 2에서 추가
  pinned         사용자가 "개발 서버로 고정"한 항목인지. Phase 2에서 추가
  title          프로젝트 이름·서비스 이름·컨테이너 이름·실행 파일 이름 중 하나
  category       dev-server | service | dev-tool | app | system
  manager        Manager
  listeners[]    { port, address, family, pid }
  processes[]    { pid, ppid, pgid, uid, user, tty, startedAt, command }   최상위와 그 아래 프로세스 (Phase 1에서 pgid·uid·tty 추가)
  project        { name, path } 또는 없음
  domains[]      이 항목에 닿는 주소 (Phase 4에서 모양을 정했다, "도메인 연결" 절)
    name         주소 이름 (예: shop.example.test, localhost)
    via          nginx (이 항목이 nginx이고 upstream으로 넘긴다) | proxied (nginx를 거쳐 이 항목에 닿는다) | direct (이 항목이 80/443을 직접 연다)
    scheme       http | https
    port         브라우저가 쓰는 포트 (nginx의 listen 포트, direct는 이 항목의 포트)
    path         nginx location 경로 (기본 /)
    upstream     nginx가 넘기는 루프백 포트 (direct는 없음)
    inHosts      hosts가 이 이름을 루프백으로 가리키는지 (localhost는 항상 true)
    tls          Cert 또는 없음 (https일 때만)
  protection     멈추기를 막는 이유 (막지 않으면 없음)
  stopPlan       멈추기 계획 또는 없음(보호 항목, 멈추는 방법이 없는 항목). Phase 3에서 추가
    method       { kind: signal, pid, group } | { kind: brew-services, brew, formula, domain } | { kind: docker-stop, docker, socket, containerId }
    command      확인 창에 보여 줄 명령 (예: brew services stop nginx)
    admin        관리자 권한이 필요한지

Cert             루프백 TLS 확인으로 읽은 인증서 (Phase 4)
  subject        주체 CN
  sans[]         SAN의 DNS 이름
  issuer         발급자 O(없으면 CN)
  mkcert         mkcert 개발 CA가 발급했는지
  notAfterMs     만료 시각
  warnings[]     만료됨 · 14일 안에 만료 · 주소 이름이 인증서에 없음 (문장)
  error          읽지 못한 이유 (있으면 나머지는 비어 있다)

Manager
  kind           launchd-brew | launchd-other | docker | pm2 | terminal | detached | app | launchd-socket | unknown
  domain         system | user (launchd일 때)
  label          launchd 라벨 (예: sh.brew.nginx)
  formula        brew 포뮬러 이름 (예: nginx)
  container      Docker 컨테이너 이름 (Phase 3)
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
| `/bin/ps -ww -axo pid=,ppid=,pgid=,uid=,tty=,user=,lstart=,command=` | 부모·프로세스 그룹·uid·터미널·사용자·시작 시각·명령줄 | 명령줄 읽기는 확인됨. 열 형식은 Phase 1에서 확인 |
| `/bin/launchctl print system` | root 서비스의 PID → 라벨 표 | 확인된 사실 |
| `/bin/launchctl print gui/<내 uid>` | 사용자 서비스의 PID → 라벨 표 | 형식은 CI 실행기에서 확인. 실제 사례는 DC-11 |
| `/bin/launchctl print <도메인>/<라벨>` | plist 경로·실행 파일·인수·작업 폴더 | 확인된 사실. 서비스 항목에만 실행 |
| `/usr/sbin/lsof -a -p <PID 목록> -d cwd -Fn` | 내 프로세스의 작업 폴더 | 자기 프로세스만 보인다는 사실 확인 |
| `<Docker.app>/Contents/Resources/bin/docker --host unix://<홈>/.docker/run/docker.sock ps --no-trunc --format '{{json .}}'` | 컨테이너 ID·이름·포트 연결 | Docker Desktop 포트 전달 프로세스가 포트를 열었을 때만. 형식 확인(DC-14) |
| `/etc/hosts` 읽기 | 루프백을 가리키는 도메인 | Phase 4. 모든 사용자가 읽을 수 있다 |
| nginx 설정 파일 읽기(`include` 포함) | server·listen·server_name·location·proxy_pass·upstream | Phase 4, DC-15. nginx master가 실행 중일 때만 |
| 루프백 TLS 연결(rustls, SNI) | 인증서 | Phase 4. https 주소에만, 결과는 60초 동안 재사용 |

`-ww`가 필요하다: BSD `ps`는 출력이 터미널이 아니면 폭을 79칸으로 잡아 명령줄을 자른다. 사용자가 터미널에서 본 결과가
잘리지 않은 것은 표준 오류가 터미널이었기 때문이고, 앱에서 실행하면 터미널이 없다.

수집은 두 번에 나눈다. 먼저 netstat·ps·launchctl 목록 두 개를 함께 실행한다. 코어가 그 결과로 상세가 필요한
launchd 라벨과 작업 폴더가 필요한 PID를 정하면, 그것만 `launchctl print <도메인>/<라벨>`과 `lsof` 한 번으로 더 읽는다.
작업 폴더에서 프로젝트를 찾는 파일 확인은 수집기가 하고, `package.json`의 `name`을 읽는 해석은 코어가 한다.
Docker Desktop의 포트 전달 프로세스가 포트를 열었으면 같은 두 번째 단계에서 `docker ps`를 한 번 실행한다.

`docker`는 포트 전달 프로세스의 앱 경로에서 찾은 CLI를 쓰고, 소켓을 `--host unix://<홈>/.docker/run/docker.sock`으로 고정한다.
사용자가 Docker context를 원격 호스트로 바꿔 두면 `--host` 없는 명령이 네트워크로 나가기 때문이다(외부 네트워크 금지 정책).

### netstat 읽기

- 헤더 행에서 열 이름으로 위치를 찾는다. 고정 위치를 쓰지 않는다(macOS 버전마다 다르다).
- `Local Address`·`Foreign Address`처럼 띄어쓰기가 있는 열 이름은 한 열로 합친다.
- `LISTEN` 행만 쓴다. 주소의 마지막 `.` 뒤가 포트다(`127.0.0.1.53733`, `*.8080`).
- `tcp4`·`tcp6`·`tcp46`을 모두 읽고, 같은 PID·포트는 한 listener로 합친다.
- 주소가 `127.0.0.1`·`::1`이면 "이 Mac에서만", `*`이면 "네트워크에서 접근 가능"으로 표시한다.
- netstat의 PID는 소켓을 마지막으로 다룬 프로세스일 수 있다(nginx worker 등). 그래서 항상 부모를 따라 올라가 최상위를 찾는다.

## 판별 순서

listener마다 `ps` 정보로 부모를 따라 올라가 조상 목록을 만든다. 목록의 맨 위는 부모가 PID 1인 프로세스(뿌리)다.
그다음 아래 순서로 관리 주체를 정한다. 먼저 맞는 규칙을 쓴다.

1. PID가 1이면 `launchd-socket`이다. launchd가 직접 연 포트이며 항상 보호한다.
2. 뿌리가 launchd PID 표(system·user)에 있으면 launchd 서비스이고, 뿌리가 최상위다.
   단 라벨이 `application.`으로 시작하면 서비스가 아니라 앱 실행 기록이므로 건너뛴다(Terminal·VS Code 같은 앱도 표에 있다).
   상세 정보의 실행 파일이 brew 경로(`/opt/homebrew/`, `/usr/local/Cellar/`, `/usr/local/opt/`) 아래면 `launchd-brew`, 아니면 `launchd-other`다.
   포뮬러 이름은 실행 파일 경로의 `opt/<이름>/` 또는 `Cellar/<이름>/`에서 얻는다.
3. listener나 그 조상이 Docker Desktop 포트 전달 프로세스(`…/Docker.app/Contents/MacOS/com.docker.backend`)이고
   `docker ps`의 포트 연결에 그 포트가 있으면 `docker`다(DC-14). 컨테이너마다 Entry 하나이고, 최상위는 listener 프로세스다.
   맞는 컨테이너가 없거나 `docker ps`가 실패하면 다음 규칙으로 넘어간다(앱 `Docker`로 보인다).
   `Ports`는 `0.0.0.0:5432->5432/tcp, [::]:5432->5432/tcp`처럼 쉼표로 나뉜다. `tcp`만 쓰고, 호스트 포트 범위
   (`127.0.0.1:8000-8001->8000-8001/tcp`)는 펼치며, 호스트 포트 없이 `5432/tcp`만 있는 연결은 건너뛴다.
4. 조상에 pm2 데몬(`PM2 … God Daemon`)이 있으면 `pm2`다. 데몬 바로 아래 프로세스가 최상위다.
5. 조상에 셸(`zsh`·`bash`·`sh`·`fish` 등)이 있으면 `terminal`이다. 뿌리에 가장 가까운 셸을 찾고, 그 아래로 셸이 이어지면 건너뛴 뒤
   처음 나오는 셸이 아닌 프로세스를 최상위로 삼는다. `npm run dev` 아래의 `sh -c vite` 같은 셸은 이 규칙에 영향을 주지 않는다.
6. 조상에 `.app/Contents/` 경로의 프로세스가 있으면 `app`이다. listener 자신이 앱 프로세스면 가장 위의 앱 프로세스가 최상위이고,
   앱 아래에서 띄운 일반 프로세스(예: 에디터가 띄운 서버)면 가장 아래 앱 프로세스의 바로 아래 프로세스가 최상위다.
7. 뿌리의 부모가 PID 1인데 위 규칙에 맞지 않으면 `detached`다(`nohup`·`&`·`disown`으로 띄운 경우). 뿌리가 최상위다.
8. 나머지는 `unknown`이다.

규칙 5~8로 정한 최상위가 listener와 다른 프로세스 그룹이면, 조상 중 listener와 같은 그룹인 가장 위 프로세스로 내려온다(Phase 3).
코딩 도구가 자기 셸(`zsh -c …`)에서 서버를 띄우면 규칙 5는 터미널 셸 바로 아래의 도구를 최상위로 잡는다. 그러면 서버를 멈출 때
도구까지 멈춘다(2026-11-19 사용자 Mac에서 Claude Code 아래 서버로 확인). 셸은 작업마다 프로세스 그룹을 만들므로 `npm run dev`는
npm까지 같은 그룹이라 그대로다.

라벨 이름 규칙(`homebrew.mxcl.*`, `sh.brew.*`)이나 `ps` 명령줄 글자로 서비스를 맞추지 않는다(DC-09).
같은 최상위 프로세스에 속한 listener는 한 Entry로 묶는다.

### 분류

분류 규칙은 설계 제안이며 Phase 1에서 합성 자료로 확정한다(DC-12).

| 분류 | 규칙 (제안) | 기본 화면 |
| --- | --- | --- |
| system | `launchd-socket`; 보안 프로그램 목록과 맞음(아래 설명); system 도메인의 `launchd-other`;
`com.apple.` 라벨; `app`(앱 자체)·`detached`·`unknown` 중 실행 파일이 `/System/`·`/usr/libexec/`·`/usr/sbin/`·`/sbin/` 아래 | 접힘, 멈추기 금지 |
| service | `launchd-brew`, user 도메인의 `launchd-other` | 펼침 |
| dev-server | `terminal`·`detached`·`pm2`·`docker`·앱 아래에서 띄운 프로세스이고, 루프백 임시 포트만 열지 않음 | 펼침 |
| dev-tool | 위 dev-server 조건이지만 루프백 주소(`127.*`, `::1`)의 임시 포트(49152~65535)만 엶(도구 사이 통신으로 추정). user 도메인 `launchd-other`도 같다. 프로젝트를 찾지 못한 `detached`(작업 폴더가 `/`인 데몬 등)도 여기에 둔다 | 접힘 |
| app | `.app` 번들 프로세스 자체가 포트를 엶 | 접힘 |

system 도메인의 `launchd-other`를 system으로 보는 이유: 개발용 root 서비스는 대부분 brew로 띄우고(`launchd-brew`),
그 밖의 system 도메인 서비스는 보안 에이전트 같은 외부 프로그램이었다(2026-11-18 관찰: root로 도는 21300·29919·39919번 등).

보안 프로그램 목록은 기본값을 두고 사용자가 고칠 수 있게 한다(설정 화면은 Phase 2). 기본값은 널리 알려진 국내 금융·인증 보안
프로그램과 제조사 이름(예: veraport, CrossEX, AnySign, nProtect, TouchEn, AhnLab, iniLINE, Interezen)이다.
listener와 그 조상의 명령줄 전체, launchd 라벨, 서비스 실행 파일 경로 중 하나에 대소문자 구분 없이 포함되면 맞는 것으로 본다.
명령줄의 첫 칸만 보면 `/Library/Application Support/…`처럼 공백이 있는 경로가 잘려 맞지 않는다(2026-11-19 사용자 Mac에서 발견).
사용자는 항목을 "개발 서버로 고정"하거나 숨길 수 있다(Phase 2).

### 보호 이유

판별 단계에서 `protection`을 채운다. 멈추기 버튼은 Phase 3에서 이 값을 따른다.

| 조건 | 이유 문구 |
| --- | --- |
| `launchd-socket` | launchd가 직접 연 포트입니다. |
| system 분류 | 시스템·보안 프로그램입니다. |
| 최상위가 portwatch 자신 | portwatch 자신입니다. |
| 최상위가 root·현재 사용자가 아닌 사용자 | 다른 사용자의 프로세스입니다. |
| 최상위가 root이고 `launchd-brew`가 아님 | root로 실행 중인 프로세스입니다. |
| user 도메인 `launchd-other` | brew가 아닌 launchd 서비스는 멈추지 않습니다(DC-13). |
| `pm2` | pm2 항목은 portwatch에서 멈추지 않습니다. 터미널에서 pm2 stop으로 멈추세요(DC-22). |

### 진단 도구

`portwatch-probe`는 화면 없이 수집을 한 번 실행한다. 기본 출력은 분류별 표, `--json`은 스냅샷 JSON,
`--raw`는 수집 명령의 원문이다. `--raw`는 사용자 이름을 `<user>`, 홈 경로를 `~`로 바꾸고, ps·launchctl 목록은
listener와 그 조상에 해당하는 줄과 형식 확인용 앞부분만 남긴다. 사용자가 출력 형식을 공유할 때 쓴다.

### 프로젝트 이름

`lsof`로 얻은 작업 폴더에서 위로 올라가며 `package.json`·`.git`·`pyproject.toml`·`Cargo.toml`·`go.mod`·`pom.xml`·`build.gradle`·
`build.gradle.kts`·`Gemfile`·`composer.json`을 찾는다. 가장 가까운 폴더가 프로젝트다. 그 폴더의 `package.json`에 `name`이 있으면
그 이름을, 없으면 폴더 이름을 쓴다. 홈 폴더와 `/`에는 올라가지 않는다. listener 프로세스의 작업 폴더를 먼저 쓰고, 없으면 최상위의 것을 쓴다.
launchd 서비스와 앱 자체는 프로젝트를 표시하지 않는다. root 서비스는 작업 폴더를 읽을 수 없고, brew 서비스의 작업 폴더
(`/opt/homebrew/var` 등)는 Homebrew 저장소 안이라 프로젝트로 잡으면 틀린다.

항목 제목은 brew 서비스면 포뮬러, 그 밖의 launchd 서비스면 라벨, 앱이면 앱 이름, Docker면 컨테이너 이름, 나머지는 프로젝트 이름이고 없으면
listener 실행 파일 이름이다. 단 dev-tool 분류는 listener 실행 파일 이름을 제목으로 하고 프로젝트는 따로 표시한다
(예: 프로젝트 폴더에서 실행한 코딩 도구가 그 프로젝트 이름으로 보이지 않게). 항목은 분류 순서(개발 서버, 서비스, 개발 도구, 앱, 시스템)와 가장 작은 포트 순으로 정렬한다.

## 멈추기

### 관리 주체별 계획

멈추기 계획(`stopPlan`)은 코어가 판별 때 함께 만든다. 보호 항목에는 계획이 없다.
아래 표는 Phase 3 설계다. DC-13·DC-14·DC-22는 2026-11-19에 정했고, DC-17은 사용자 Mac 확인(M-08)으로 닫는다.

| 관리 주체 | 실행 (확인 창에 보이는 명령) | 권한 | 비고 |
| --- | --- | --- | --- |
| `terminal`·`detached`·`app`(앱 아래 프로세스와 앱 자체)·`unknown` | 최상위가 자기 프로세스 그룹의 리더(pgid = PID)면 그룹에 SIGTERM(`kill -TERM -<pgid>`), 아니면 최상위 PID에만(`kill -TERM <pid>`). 남아 있으면 사용자 확인 후 같은 대상에 SIGKILL | 일반 | nodemon 같은 감시 도구까지 함께 멈춘다. 그룹 리더가 아니면 셸이 같은 그룹일 수 있어 그룹에 보내지 않는다 |
| `launchd-brew` (user) | `<brew> services stop <포뮬러>` (`brew services stop nginx`) | 일반 | kill 하면 KeepAlive가 다시 띄울 수 있다 |
| `launchd-brew` (system) | 관리자 권한으로 `/usr/bin/env HOME=/var/root HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_ANALYTICS=1 HOMEBREW_NO_ENV_HINTS=1 <brew> services stop <포뮬러>` | 관리자 | DC-17 제안. root brew는 `services`만 허용하고 `system` 도메인에서 bootout한 뒤 `/Library/LaunchDaemons`의 plist를 지운다 |
| `launchd-other` | 없음 | — | DC-13: 멈추지 않는다 |
| `docker` | `<docker> --host unix://<홈>/.docker/run/docker.sock stop <컨테이너 ID>` (`docker stop <이름>`) | 일반 | DC-14 |
| `pm2` | 없음 | — | DC-22: 멈추지 않는다(사용자 결정) |
| `launchd-socket`·system·보호 항목 | 없음 | — | 버튼을 보여주지 않는다 |

- `<brew>`는 서비스 실행 파일이 `/opt/homebrew/` 아래면 `/opt/homebrew/bin/brew`, `/usr/local/` 아래면 `/usr/local/bin/brew`다. 다른 경로는 쓰지 않는다.
- `<docker>`는 포트 전달 프로세스 경로의 `…/Docker.app/Contents/Resources/bin/docker`다. 컨테이너는 이름이 아니라 ID로 멈춘다.
- 신호는 셸을 거치지 않고 시스템 호출로 보낸다. PID·pgid가 0·1이면 보내지 않는다.

### 흐름

1. 사용자가 멈추기를 누르면 같은 창 안의 확인 칸에 대상·관리 주체·실행할 명령·필요한 권한을 보여준다.
2. 화면은 항목 `id`만 보낸다. 셸은 마지막 수집 결과에서 그 항목의 계획을 찾아 쓰고, 화면이 보낸 명령이나 값은 쓰지 않는다.
3. 실행 직전에 최상위 프로세스의 PID·시작 시각·명령줄을 `ps`로 다시 읽어 스냅샷과 같은지 확인한다. 다르면 멈추지 않고 "바뀜"으로 알린다(PID 재사용 방지).
   계획의 값(포뮬러·컨테이너 ID·경로)도 실행 직전에 다시 검증한다.
4. 계획을 실행한다. 한 번에 한 항목만 멈춘다(실행 중 다른 요청은 거절).
5. 0.5초 간격으로 최대 5초 동안 다시 수집해 결과를 정한다. 사라진 것이 보여도 2초가 될 때까지는 계속 본다(다시 뜨는지 보기 위해).
   - 같은 `id`의 항목이 남아 있으면 "아직 실행 중"이다. 신호 계획이면 강제 종료(SIGKILL)를 제안한다.
   - 같은 `key`의 다른 항목(새 프로세스)이 있으면 "다시 실행됨: <관리 주체>"다.
   - 둘 다 없으면 "멈춤"이다.
   판정은 코어의 순수 함수로 둔다.
6. 결과는 그 항목 자리(없어졌으면 목록 위 상태 줄)에 보이고, 창을 닫았다 열어도 다음 결과 전까지 남는다.
   관리자 비밀번호 창이 뜨면 팝오버가 포커스를 잃고 닫히므로, 결과는 창을 다시 열어 본다.

### 관리자 권한

- 앱은 일반 사용자 권한으로만 실행한다. 관리자 권한은 확인한 명령 하나에만 쓴다.
- macOS 관리자 비밀번호 창(`/usr/bin/osascript -e 'do shell script "<명령>" with administrator privileges'`)을 쓴다.
  비밀번호는 macOS 창이 받고 앱은 다루지 않는다.
- 명령 문자열은 고정 틀에 검증한 값만 넣어 만든다. 포뮬러는 `^[a-z0-9@+._-]+$`, 컨테이너 ID는 소문자 16진수 12~64자,
  PID는 숫자만 허용한다. 값에 따옴표·공백·`;`·`$`·백슬래시가 들어갈 수 없으므로 AppleScript 문자열과 `/bin/sh` 명령줄을 벗어나지 못한다.
- 사용자가 비밀번호 창을 취소하면(osascript 오류 -128) 아무것도 바꾸지 않고 "취소됨"으로 표시한다.
- 권한 상승이 실패하면 `sudo <명령>`을 복사해서 터미널에서 실행할 수 있게 보여준다.
- 비밀번호 입력을 기다리므로 osascript 제한 시간은 120초, 일반 멈추기 명령은 30초다.
- `sudo brew services start`가 root 소유로 바꾼 파일은 멈춘 뒤에도 root 소유로 남는다. 앱은 소유자를 되돌리지 않는다(수동 검증 정리 절차).

### 보호 항목

다음은 멈추기 버튼을 보여주지 않는다: PID 1, `launchd-socket`, system 분류, 보안 프로그램 목록,
portwatch 자신, root·현재 사용자가 아닌 다른 사용자의 프로세스(서비스로 판별된 root 서비스는 예외).

## 도메인 연결

Phase 4 설계. 파싱과 연결은 코어(`hosts.rs`·`nginx.rs`·`domains.rs`), 파일 읽기와 TLS 연결은 수집기가 한다.

1. `/etc/hosts`에서 `127.0.0.0/8`·`::1`을 가리키는 이름을 모은다(소문자). `localhost`·`broadcasthost`와 주석은 뺀다.
2. nginx 설정을 읽는다(DC-15). 처음에는 nginx만 지원한다(Caddy 등은 이후 범위).
   - 실행 중인 nginx master(`nginx: master process <실행 파일> …`)가 있을 때만 읽는다. 설정 파일은 명령줄의 `-c <경로>`,
     없으면 실행 파일의 brew 위치에 따라 `/opt/homebrew/etc/nginx/nginx.conf` 또는 `/usr/local/etc/nginx/nginx.conf`다.
     둘 다 아니면 경고만 남긴다.
   - `include`는 설정 파일 폴더 기준 상대 경로로 풀고, 마지막 경로 조각의 `*` 패턴만 펼친다(예: `servers/*`). 이름순으로 넣는다.
     파일은 64개, 하나에 1 MB, 깊이 8까지 읽는다. 넘거나 읽지 못하면 경고를 남기고 읽은 것만 쓴다.
   - `http` 안의 `server` 블록마다 `listen`(포트와 `ssl`), `server_name`, `location <경로>`(정규식 location은 건너뜀) 안의
     `proxy_pass`를 읽는다. `listen`이 없으면 80이다. `proxy_pass`의 호스트가 루프백(`127.*`·`localhost`·`[::1]`)이면 그 포트,
     `upstream` 이름이면 그 블록의 루프백 `server`들의 포트를 쓴다. 루프백이 아닌 대상은 버린다.
   - `server_name`의 `_`·빈 이름·정규식(`~`)·와일드카드(`*`)는 버린다. 남은 이름이 없으면 `localhost`로 본다.
     `localhost`는 남긴다(경로로 나눠 쓰는 설정이 흔하다). 정적 파일만 내주는 location은 주소로 만들지 않는다.
3. 주소를 항목에 붙인다.
   - nginx 항목(listener 실행 파일이 `nginx`이고 listen 포트를 연 항목): `via: nginx`, 주소마다 location 경로와 upstream 포트.
   - upstream 포트를 연 항목: 같은 주소를 `via: proxied`로 붙인다.
   - nginx가 아닌 항목이 80·443을 직접 열면 hosts의 루프백 이름을 `via: direct`로 붙인다(443은 https).
   - 설정에만 있고 nginx 항목이 실제로 열지 않은 포트의 경로는 붙이지 않는다(설정을 고친 뒤 다시 읽기 전 등).
   - 같은 값은 한 번만 둔다. 이름·포트·경로 순으로 정렬한다.
   - nginx는 명령줄을 `nginx: master process <실행 파일> …`로 바꾸므로, 실행 파일 이름은 그 안의 경로(일꾼은 `nginx`)로 읽는다.
4. https 주소는 수집기가 `127.0.0.1:<포트>`(listener가 `::1`에만 열렸으면 `[::1]`)에 SNI로 이름을 주고 TLS 핸드셰이크만 한 뒤
   인증서를 읽는다. 인증서 검증은 하지 않고(읽기만 한다) 데이터는 보내지 않는다. 연결·핸드셰이크 제한 시간은 각 1초다.
   (포트, 이름)별 결과를 60초 동안 재사용해 갱신 주기마다 연결하지 않는다.
   코어가 DER 인증서를 읽어 주체·SAN·발급자(mkcert 여부)·만료일을 채우고, 만료·14일 안 만료·이름 불일치를 경고한다.
   SAN의 `*.example.test`는 한 단계 하위 이름과 맞는다.

설정 파일과 hosts 파일은 읽기만 한다. TLS 확인 말고는 네트워크에 연결하지 않는다.

## 화면과 갱신

Phase 2 설계. 아래 화면 그림은 배치 제안이며 멈추기 버튼은 Phase 3, 도메인은 Phase 4에서 붙는다.

```text
[메뉴바 아이콘] 2                       ← 개발 서버·서비스 중 숨기지 않은 항목 수 (0이면 숫자 없음)

portwatch                 09:41:05 갱신  [새로 고침] [설정]
▾ 개발 서버 2
  shop-web     :5173 외부   터미널
  api-server   :3000 로컬   터미널
▾ 서비스 1
  nginx        :8080 외부   brew 서비스 · root
▸ 개발 도구 4
▸ 앱 1
▸ 시스템·보안 9
▸ 숨긴 항목 1
[경고 줄]                                                     [종료]
```

### 갱신 루프

- 셸이 백그라운드 스레드 하나에서 수집한다. 창을 연 동안은 짧은 주기(기본 3초), 닫힌 동안은 긴 주기(기본 15초)로 수집한다(DC-16).
  창을 열면 바로 한 번 수집하고, 새로 고침 버튼도 바로 수집하게 깨운다. 수집이 겹치지 않게 한 스레드에서만 한다.
- 수집할 때마다 설정의 숨기기·고정을 적용하고, 메뉴바 아이콘 옆 숫자를 바꾸고, 창에 `snapshot` 이벤트를 보낸다.
- 수집이 실패해도 루프는 계속 돈다. 스냅샷은 수집기가 만든 그대로(일부 명령 실패는 `warnings`) 보낸다.

### 항목 기준(key)

숨기기·고정은 PID가 아니라 다시 실행해도 같은 기준으로 기억한다. 코어가 계산한다.

| 항목 | key |
| --- | --- |
| launchd 서비스(brew·그 밖) | `launchd:<라벨>` |
| launchd 소켓 | `launchd-socket` |
| 앱 자체 | `app:<앱 이름>` |
| Docker 컨테이너 | `docker:<컨테이너 이름>` (Phase 3) |
| 프로젝트가 있는 항목(개발 도구 제외) | `project:<프로젝트 경로>` |
| 개발 도구와 그 밖 | `exe:<listener 실행 파일 이름>` (예: 같은 도구 여러 개를 한 번에 숨긴다) |

개발 도구는 프로젝트 폴더에서 실행돼도 `exe:` key를 쓴다. 같은 프로젝트의 개발 서버와 key가 겹치면 도구를 숨길 때 서버까지 숨겨지기 때문이다.

### 설정

앱 설정 폴더(`~/Library/Application Support/io.github.superlucky84.portwatch/settings.json`)에 JSON으로 저장한다.
파일이 없거나 읽을 수 없으면 기본값으로 시작하고 경고를 남긴다.

| 값 | 기본 | 범위 |
| --- | --- | --- |
| `refreshOpenSecs` | 3 | 2, 3, 5 |
| `refreshClosedSecs` | 15 | 10, 15, 30, 60 |
| `hidden` | 빈 목록 | 항목 key 목록 |
| `pinned` | 빈 목록 | 항목 key 목록. 분류를 개발 서버로 바꿔 보여 준다. 보호는 그대로다 |
| `securityNames` | 없음(기본 목록 사용) | 보안 프로그램 이름 조각 목록. 설정하면 기본 목록 대신 쓴다 |

로그인 시 자동 실행은 설정 파일이 아니라 자동 실행 플러그인의 상태를 그대로 쓴다.
설정 값의 모양과 범위 정리, 숨기기·고정 적용, 아이콘 숫자 계산은 코어의 순수 함수로 둔다.

### 화면에 노출하는 명령

| 명령 | 하는 일 |
| --- | --- |
| `get_state` | 마지막 스냅샷(없으면 없음)·설정·자동 실행 상태 |
| `refresh_now` | 갱신 루프를 바로 깨운다 |
| `set_hidden(key, hidden)`·`set_pinned(key, pinned)` | 설정을 바꾸고 마지막 스냅샷에 다시 적용해 보낸다 |
| `update_settings(값)` | 갱신 주기·보안 프로그램 목록을 바꾼다. 범위 밖 값은 거부한다 |
| `set_launch_at_login(enabled)` | 로그인 시 자동 실행을 켜고 끈다 |
| `open_url(url)` | `http(s)://localhost` 또는 `127.0.0.1`에 포트만 붙은 주소, 또는 마지막 스냅샷의 주소 중 `inHosts`인 것의 URL만 기본 브라우저로 연다(`/usr/bin/open`). hosts에 없는 이름은 바깥으로 나갈 수 있어 열지 않는다(Phase 4) |
| `copy_text(text)` | 2,000자 이하 글자를 클립보드에 넣는다(`/usr/bin/pbcopy`) |
| `stop_entry(id)` | 마지막 수집 결과의 그 항목을 계획대로 멈추고 결과를 돌려준다(Phase 3, "멈추기" 절) |
| `force_stop_entry(id)` | 신호 계획 항목에만 같은 대상에 SIGKILL을 보낸다(Phase 3) |
| `hide_popover`·`quit_app` | 창 닫기, 앱 종료 |

### 화면

- 프레임워크 없이 ES 모듈로 만든다(DC-20). 받은 스냅샷을 그리기만 한다.
- 분류 순서는 개발 서버, 서비스, 개발 도구, 앱, 시스템·보안, 숨긴 항목이다. 개발 서버·서비스만 기본으로 펼친다.
- 행을 누르면 상세가 펼쳐진다: 프로세스 목록(PID·사용자·명령줄), 프로젝트 경로, 시작 시각, 라벨, 보호 이유.
  상세의 버튼: 열기·URL 복사(개발 서버·서비스이고 포트가 있을 때, `http://localhost:<가장 작은 포트>`), 숨기기/다시 보이기,
  개발 서버로 고정/고정 해제, 멈추기(계획이 있고 보호가 없을 때, Phase 3).
- 주소(Phase 4): 행 제목 옆에 대표 이름(localhost가 아닌 이름 중 hosts에 있는 것 먼저)을 작게 보이고, 인증서 경고가 있으면
  "인증서" 표시를 붙인다. 상세에는 주소마다 URL, 경유(`→ :<upstream>`·"nginx 경유"·"직접"), hosts에 없으면 "hosts에 없음",
  인증서(발급자, mkcert면 표시, 만료일) 또는 읽지 못한 이유, 경고를 보이고 열기(hosts에 있을 때만)·복사 버튼을 둔다.
- 멈추기를 누르면 상세 안에 확인 칸(대상·관리 주체·명령·"관리자 비밀번호가 필요합니다" 또는 "일반 권한")과 멈추기·취소 버튼이 열린다.
  실행 중에는 "멈추는 중…"을, 끝나면 결과(멈춤·다시 실행됨·아직 실행 중·취소됨·바뀜·실패)를 보인다.
  실패하면 터미널용 명령 복사 버튼을, "아직 실행 중"이고 신호 계획이면 강제 종료 버튼(한 번 더 확인)을 보인다.
  `stop_entry` 결과 모양: `{ outcome, message, command, canForce }`. `outcome`은 `stopped | restarted | still-running | cancelled | changed | failed`.
- 갱신 때 펼친 행·접은 분류·스크롤 위치를 유지한다. 내용(수집 시각 제외)이 같으면 다시 그리지 않는다.
  펼침 상태는 항목 key로 기억하고, 앱을 다시 켜도 남도록 웹뷰 저장소에 둔다(읽기 실패는 무시).
- Esc로 창을 닫는다. 설정은 같은 창의 설정 화면에서 바꾼다.
- 화면 문자열은 한국어다.

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
- [x] DC-03: 최소 지원 macOS는 15.0이다. 2026-11-18 사용자 Mac이 15.6.1이고, netstat·launchctl 출력 형식을 이 버전에서만 확인했기 때문이다.
  더 낮은 버전은 동료 배포가 필요해지면 그 버전의 출력 형식을 확인한 뒤 낮춘다. `bundle.macOS.minimumSystemVersion`에 적용한다. 검증: Phase 0, M-13.
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
- [x] DC-10: CI는 Linux 작업(검사·코어 테스트)을 모든 main 푸시와 PR에서 돌리고, macOS 빌드는 수동 실행(`workflow_dispatch`)에서만 돌린다.
  근거(2026-11-18 조사): 표준 macOS 실행기는 분당 약 $0.062로 Linux(약 $0.006)의 10배 정도다. 무료 한도는 자료마다 달라 확인하지 못했다.
  Phase 0 실측: 캐시 없는 macOS 작업 전체가 약 1분 45초였다(run 44276588647). 코드가 커지면 다시 잰다.
  사용자가 비용 기준을 바꾸면 이 결정을 다시 연다. 검증: Phase 0 CI 실행 기록.
- [x] DC-11: 사용자 서비스 목록 `launchctl print gui/<uid>`는 system과 같은 형식이고 같은 파서로 읽는다.
  근거: 2026-11-18 macOS 15.8.2 CI 실행기(run 44282667243)와 2026-11-19 사용자 Mac에서 `services = { … }` 블록과
  `PID 상태<탭>라벨` 줄 형식이 같았다. 상태 칸에는 `-` 말고 `(pe)` 같은 값도 온다. 사용자 Mac에서 사용자 도메인 LaunchAgent 3개가
  `launchd-other`(user)로 판별됐다. brew 사용자 서비스는 같은 경로(사용자 목록 → 서비스 상세 → brew 실행 경로)로 판별하며
  실제 사례는 M-02(Phase 7)에서 본다.
- [x] DC-12: 분류 규칙은 위 "분류" 절대로 정한다(임시 포트 49152~65535의 루프백만 열면 dev-tool, 보안 프로그램은 명령줄 전체·라벨·서비스
  실행 경로로 비교, system 도메인 `launchd-other`는 system, 프로젝트 없는 `detached`는 dev-tool, 기본 보안 목록은 국내 금융·보안
  프로그램과 제조사 이름). 근거: 합성 자료 시나리오와 사용자 Mac 두 차례 결과. 2026-11-19 1차 결과로 네 가지를 고쳤고,
  2차 결과(run 44441488154)에서 기본으로 펼치는 개발 서버·서비스에 모노레포 개발 서버와 root nginx만 남았다. 분리 실행 데몬과
  코딩 도구 3개는 개발 도구, 보안 프로그램 6개·Apple 서비스·launchd 소켓은 시스템·보안으로 접혔다. 사용자가 고칠 수 있는 목록과
  "개발 서버로 고정"·숨기기는 Phase 2에서 만든다. 검증: M-04.
- [x] DC-13: brew가 아닌 launchd 서비스(`launchd-other`)는 지금은 멈추지 않는다. 2026-11-19 사용자 결정("지금은 멈추지 않음").
  근거: 사용자 Mac의 brew 외 LaunchAgent·LaunchDaemon은 업데이터와 보안 프로그램뿐이어서 멈출 이유가 적고, 잘못 멈추면 위험하다.
  보호 이유로 안내한다. 개발용 launchd 서비스를 쓰게 되면 다시 연다(사용자 LaunchAgent만 `launchctl bootout gui/<uid>/<라벨>`로 허용하는 안).
  검증: M-10.
- [x] DC-14: Docker Desktop은 `…/Docker.app/Contents/MacOS/com.docker.backend services` 프로세스가 컨테이너의 호스트 포트를 연다
  (부모는 같은 앱의 `com.docker.backend`, 그 부모는 PID 1, launchd 표에는 `application.` 라벨). 컨테이너는 `docker ps`의 `Ports`
  (`0.0.0.0:18080->8080/tcp, 127.0.0.1:18081->8081/tcp`)의 호스트 포트로 맞추고, `docker stop <ID>`로 멈춘다.
  근거: 2026-11-19 사용자 Mac(Docker Desktop, 엔진 28.1.1)에서 시험 컨테이너로 확인했다. 환경을 비운 상태에서도 `docker ps`가 동작했다.
  OrbStack·colima는 사용자 Mac에 없어 지원하지 않는다(앱이나 분리 실행으로 보인다). 쓰게 되면 그때 형식을 확인한다. 검증: F-05, M-11.
- [x] DC-15: nginx 설정은 파일을 직접 읽는다("도메인 연결" 절). `nginx -T`는 쓰지 않는다.
  근거(2026-11-19, 사용자 Mac의 brew nginx 1.27.5): 설정 파일은 사용자 소유로 읽을 수 있다. `nginx -T`는 설정 시험 중에 오류 로그를
  쓰기로 열어서 파일을 만들 수 있고(읽기만 한다는 원칙), `sudo brew services start`로 띄운 뒤에는 로그가 root 소유가 되어 일반 권한으로
  실패한다. 직접 읽기는 실행 파일을 실행하지 않는다. 사용자 설정은 `server_name localhost`에 location 경로별로 루프백 포트에 넘기는
  모양이어서 `localhost`도 주소로 남긴다. 검증: F-07, M-12.
- [x] DC-16: 갱신 주기 기본값은 창을 연 동안 3초, 닫은 동안 15초이고 설정에서 2·3·5초와 10·15·30·60초 중 고른다.
  근거: 2026-11-19 사용자가 이 기본값으로 Phase 2 앱을 써서 목록·숫자 갱신이 문제없다고 확인했고, macOS CI 실행기에서 수집 한 번이
  110 ms였다(항목 1개). 사용자 Mac의 수집 시간·CPU 수치는 받지 않았다. CPU 부담이 보고되면 닫힌 동안의 기본값부터 늘린다. 검증: M-05.
- [ ] DC-17: root brew 서비스를 관리자 권한으로 멈추는 실제 명령. 제안: 관리자 권한으로 `/usr/bin/env HOME=/var/root
  HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_ANALYTICS=1 HOMEBREW_NO_ENV_HINTS=1 <brew> services stop <포뮬러>`("멈추기" 절).
  근거(2026-11-19, 사용자 Mac의 Homebrew 7.0.14 소스): `brew.sh`의 root 검사는 `services` 명령을 허용하고, root일 때 `brew services stop`은
  `system` 도메인에서 `launchctl bootout`한 뒤 `/Library/LaunchDaemons`의 plist를 지운다. `launchctl bootout`만 쓰면 plist가 남아
  재부팅 때 다시 뜬다. `HOME=/var/root`는 root brew가 사용자 홈의 캐시에 root 소유 파일을 만들지 않게 하려는 것이다.
  남은 확인: osascript 관리자 권한에서 실제로 멈추는지, 취소 처리. 사용자 Mac M-08로 닫는다.
- [ ] DC-18: 배포 위치. TBD — 사용자 결정 필요. 비공개 저장소의 Releases는 Homebrew가 인증 없이 받을 수 없다.
  - (a) portwatch 저장소를 공개로 바꾼다. 가장 단순하다.
  - (b) 코드는 비공개로 두고 빌드한 앱만 공개 저장소(예: `superlucky84/homebrew-tap`의 Releases)에 올린다.
  - (c) brew 없이 Mac에서 직접 빌드해 `/Applications`에 설치한다. Gatekeeper 절차가 필요 없지만 RQ-08과 다르다.
  Phase 5 전에 정한다. 검증: M-13.
- [x] DC-19: 대상 CPU는 Apple Silicon(`aarch64-apple-darwin`)만이다. 2026-11-18 사용자 Mac의 `uname -m`이 `arm64`였다.
  Intel Mac 사용자에게 배포하기로 하면 universal 빌드를 추가한다. Intel brew 경로(`/usr/local`) 판별은 Phase 6에서 합성 자료로만 확인한다.
- [x] DC-20: 화면은 프레임워크 없이 ES 모듈로 만든다(docatlas 화면과 같은 방식). 2026-11-19 Phase 2 시작 때 정했다.
  근거: 화면이 작고(목록·상세·설정) 빌드 단계 없이 `ui/`를 그대로 번들에 넣을 수 있다. 화면 테스트는 `window.__TAURI__`를
  가짜로 바꾸고 Playwright(`@playwright/test` 1.63.0)로 이 개발 환경과 Linux CI에서 돌린다. 검증: `npm run test:ui`.
- [ ] DC-21: Gatekeeper 처리. 허용 절차를 README로 안내할지, cask 설치 단계에서 격리 표시를 지울지 고른다.
  후자는 편하지만 Gatekeeper 확인을 건너뛰므로 회사 보안 정책에 맞는지 사용자가 판단해야 한다. Phase 5. 검증: M-14.
- [x] DC-22: pm2 항목은 멈추지 않는다. 목록에는 보이고, 보호 이유로 터미널의 `pm2 stop`을 안내한다.
  2026-11-19 사용자 결정("pm2는 안 써서 A로"). 근거: 사용자가 pm2를 쓰지 않고 사용자 Mac에도 없어 확인할 수 없다.
  `pm2 stop`에는 pm2 경로·node·앱 이름(`pm2 jlist`)이 필요하고, pm2 자식을 신호로 멈추면 pm2가 다시 띄운다.
  pm2를 쓰게 되면 다시 연다(`pm2 stop <이름>` 지원). 검증: 합성 자료 F-06, M-10.
