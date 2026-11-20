# portwatch 수동 검증 체크리스트

문서 기준: 2026-11-18.
[요구사항](./REQUIREMENTS.md) · [설계](./DESIGN.md) · [구현 계획](./IMPLEMENT.md)

구현된 코드가 없어서 모든 항목이 미실행이다. 수동 검증은 사용자의 회사 Mac에서 한다.
자동 테스트로 대신 확인한 부분은 각 항목에 따로 적고, 사람이 확인한 결과와 섞지 않는다.

## 합성 자료

자동 테스트는 아래 합성 자료를 쓴다. 형식은 2026-11-18 사용자 Mac 관찰을 따르되,
사용자 이름·실제 PID·사내 정보·실제 출력 원문은 넣지 않는다. 도메인은 `.test`만 쓴다.
F-01~F-07은 한 대의 가상 Mac 상태로 묶어 `crates/portwatch-core/tests/fixtures/mac-15/`에 명령별 출력 파일로 둔다.
코어의 `tests/mac_fixtures.rs`가 시나리오별로, 수집기 테스트가 같은 파일로 수집 흐름 전체를 검증한다. 사용자 이름은 `dev`, uid는 501이다.

| ID | 상황 | 들어가는 출력 |
| --- | --- | --- |
| F-01 | root brew nginx: master(root, 부모 1)와 worker(`nobody`), `*.8080` | netstat·ps·`launchctl print system`·`launchctl print system/sh.brew.nginx` |
| F-02 | 사용자 brew 서비스(예: 6379 redis) | netstat·ps·`launchctl print gui/<uid>` (형식은 DC-11 확인 뒤 확정) |
| F-03 | 터미널 개발 서버 두 개: `zsh → npm → sh → node`(5173), nodemon 아래 node(3000), 서로 다른 `package.json` 이름 | netstat·ps·lsof 작업 폴더 |
| F-04 | 잡음: launchd가 연 22번, Kerberos 88번(root), 가상의 보안 프로그램(루프백), 개발 도구의 루프백 임시 포트, `.app` 번들 프로세스 | netstat·ps·`launchctl print system` |
| F-05 | Docker Desktop 컨테이너 두 개(5432, `0.0.0.0`과 `[::]`; 9000-9001 범위, 루프백)와 컨테이너에 맞지 않는 포트 하나(6443). `com.docker.backend services`가 연다 | netstat·ps·`docker ps --format '{{json .}}'` (형식은 DC-14) |
| F-06 | `nohup`으로 띄워 부모가 1이 된 node, pm2 데몬 아래 node | netstat·ps·`launchctl print system` |
| F-07 | hosts `127.0.0.1 shop.example.test`(루프백이 아닌 이름 하나 포함). F-01 nginx가 443도 열고, `nginx.conf`는 `localhost:8080`의 `/api`를 3000으로(사용자 Mac 설정 모양), `servers/*`의 `shop.conf`는 `listen 443 ssl`·`upstream`으로 5173에 넘긴다. 루프백이 아닌 대상 하나 포함. 인증서는 테스트가 rcgen으로 만든다 | `hosts.txt`·`nginx/` 설정·netstat |

## 시험 준비와 정리

- 같은 포뮬러를 사용자 서비스와 root 서비스로 동시에 띄우지 않는다. 하나를 멈춘 뒤 다른 쪽을 띄운다.
- `sudo brew services start`는 해당 포뮬러의 실행 파일과 링크 소유자를 `root:admin`으로 바꾼다.
  root 서비스 시험이 끝나면 아래 순서로 정리한다.

```bash
sudo brew services stop nginx
```

```bash
find /opt/homebrew -user root 2>/dev/null
```

목록에 시험한 포뮬러의 경로만 있을 때 본인 소유로 되돌린다.

```bash
find /opt/homebrew -user root 2>/dev/null | xargs sudo chown -h "$(whoami)":admin
```

`sudo brew`는 사용자 홈의 Homebrew 캐시에도 root 소유 파일(예: `tool-selection/curl`·`git`)을 만들 수 있다. 확인하고 지운다(brew가 다시 만든다).

```bash
find ~/Library/Caches/Homebrew -user root 2>/dev/null
```

소유자 변경은 `sudo brew services start`가 만든다. portwatch의 멈추기는 소유자를 바꾸지도 되돌리지도 않는다.

### M-01 — root brew 서비스 탐지

대응: RQ-03·04, DC-07·08·09. 예제: F-01.

- [ ] `sudo brew services start nginx`로 nginx를 띄운다.
- [ ] 목록의 서비스 분류에 nginx가 하나의 항목으로 보인다(worker가 따로 보이지 않는다).
- [ ] 항목에 포트 8080, `brew 서비스`, `root` 표시가 있다.
- [ ] 상세에 master PID, worker, 라벨, plist 경로, 실행 파일이 보인다.
- [ ] 시험 뒤 위의 정리 절차를 실행한다.

통과: nginx가 root brew 서비스로 정확히 표시된다. 상태: 탐지는 2026-11-19 사용자 Mac에서 `portwatch-probe`로 확인(한 항목, root, master·worker). 화면 표시는 Phase 2 이후. 2026-11-20 로컬 빌드 앱에서 root 표시와 함께 확인(M-08 준비).

### M-02 — 사용자 brew 서비스 탐지

대응: RQ-04, DC-11. 예제: F-02.

- [ ] sudo 없이 `brew services start <포뮬러>`로 서비스를 띄운다.
- [ ] 항목에 `brew 서비스` 표시가 있고 `root` 표시는 없다.

통과: 사용자 서비스와 root 서비스가 구분된다. 상태: 탐지는 2026-11-19 사용자 Mac에서 `portwatch-probe`로 확인(nginx가 `brew 서비스(user)`, root 표시 없음). 화면 표시는 2026-11-20 로컬 빌드 앱에서 확인.

### M-03 — 터미널 개발 서버 여러 개

대응: RQ-02·04, PR-04. 예제: F-03.

- [ ] 서로 다른 두 프로젝트에서 개발 서버를 띄운다. 하나는 nodemon처럼 감시 도구 아래에서 띄운다.
- [ ] `python3 -m http.server 8000`도 띄운다.
- [ ] 세 서버가 개발 서버 분류에 각각 보인다. 프로젝트 이름, 포트, 실행 명령이 맞다.
- [ ] 감시 도구 아래 서버는 감시 도구와 함께 한 항목으로 묶인다.

통과: 포트만 다른 서버들이 프로젝트별로 구분된다. 상태: 일부 통과. 2026-11-19 probe로 터미널 서버 1개(모노레포 하위 패키지 이름, 하위 프로세스 묶음)를, Phase 2 앱으로 서로 다른 프로젝트의 서버가 프로젝트 이름으로 구분되는 것을 확인했다. 감시 도구(nodemon)·`python3 -m http.server` 경우는 2026-11-20 로컬 빌드 앱에서 확인해 통과.

### M-04 — 잡음 분류

대응: RQ-02, PR-03, DC-12. 예제: F-04.

- [ ] 보안 프로그램·Kerberos·launchd가 연 포트가 시스템·보안 분류에 접혀 있다.
- [ ] 개발 도구의 임시 포트가 개발 도구 분류에 접혀 있다.
- [ ] 펼친 기본 화면에는 개발 서버와 서비스만 있다.
- [ ] 항목을 숨기거나 "개발 서버로 고정"하면 다음 갱신에도 유지된다.

통과: 기본 화면에 개발과 관계없는 항목이 보이지 않는다. 상태: 통과(2026-11-19). 분류는 probe 2차로, 접기·숨기기·다시 보이기·고정은 Phase 2 앱으로 사용자 Mac에서 확인했다.

### M-05 — 메뉴바 표시와 자동 갱신

대응: RQ-07, PR-06·07·08, DC-05·16.

- [ ] 앱을 실행하면 메뉴바에 아이콘이 생기고 Dock에는 아이콘이 없다.
- [ ] 아이콘을 누르면 아이콘 아래에 목록이 열리고, 다른 곳을 누르면 닫힌다.
- [ ] 메뉴바 아이콘 옆 숫자가 숨기지 않은 개발 서버·서비스 수와 같고, 0이면 숫자가 없다.
- [ ] 서버를 새로 띄우거나 끄면 갱신 주기 안에 목록과 숫자에 반영된다. 창을 닫아 둔 동안에도 숫자가 바뀐다.
- [ ] 개발 서버 행의 열기로 기본 브라우저에 `http://localhost:<포트>`가 열리고, URL 복사가 클립보드에 들어간다.
- [ ] 설정 화면에서 갱신 주기와 로그인 시 자동 실행을 바꿀 수 있고, 앱을 다시 켜도 유지된다.
- [ ] Esc로 창이 닫힌다.
- [ ] 갱신 중 펼친 행과 스크롤 위치가 유지된다.
- [ ] 화면 문자열이 한국어다.

통과: 위 항목이 모두 맞다. 상태: 통과(2026-11-19, 사용자 Mac, run 44479705892의 앱). 메뉴바·Dock은 Phase 0에서, 나머지는 Phase 2에서 확인했다.

### M-06 — 터미널 서버 멈추기

대응: RQ-06, PR-01. 예제: F-03.

- [ ] M-03의 감시 도구 아래 서버를 멈춘다.
- [ ] 확인 칸에 대상·관리 주체·실행할 신호(`kill -TERM -<pgid>`)가 보인다. 취소하면 아무것도 바뀌지 않는다.
- [ ] 멈춘 뒤 감시 도구까지 끝나고 포트가 비며, 서버가 다시 뜨지 않는다.
- [ ] 띄웠던 터미널 창은 그대로 남는다.

통과: 프로세스 그룹 전체가 끝나고 결과가 "성공"이다. 상태: 통과(2026-11-20, 사용자 Mac, 로컬 빌드 앱. M-03의 감시 도구·`python3 -m http.server` 경우도 함께 확인).

### M-07 — 사용자 brew 서비스 멈추기

대응: RQ-06, PR-01. 예제: F-02.

- [ ] M-02의 서비스를 멈춘다. 확인 창에 `brew services stop <포뮬러>`가 보인다.
- [ ] 멈춘 뒤 포트가 비고, 30초 뒤에도 다시 뜨지 않는다.
- [ ] 터미널의 `brew services list`에서도 멈춘 상태다.

통과: launchd가 다시 띄우지 않는다. 상태: 통과(2026-11-20, 사용자 Mac, 사용자 brew nginx).

### M-08 — root brew 서비스 멈추기

대응: RQ-03·06, PR-02, DC-17. 예제: F-01.

- [ ] M-01의 nginx를 멈춘다. 확인 창에 관리자 권한이 필요하다고 표시된다.
- [ ] macOS 관리자 비밀번호 창에서 취소하면 아무것도 바뀌지 않고 "취소됨"이 보인다.
- [ ] 다시 시도해 비밀번호를 넣으면 nginx가 멈추고 포트가 빈다.
- [ ] 30초 뒤에도 다시 뜨지 않는다. 시험 뒤 정리 절차를 실행한다.

통과: 앱이 root로 실행되지 않은 채 root 서비스를 멈춘다. 상태: 통과(2026-11-20, 사용자 Mac). 1차는 "Need to download … but cannot as root!"로 실패했고,
`HOMEBREW_CACHE`를 사용자 캐시로 주도록 고친 뒤 취소·멈춤 모두 맞게 동작했다(DC-17).

### M-09 — 다시 실행되는 경우 감지

대응: RQ-06, PR-01. 예제: F-06.

- [ ] KeepAlive가 켜진 서비스의 프로세스를 터미널에서 직접 kill 한다. 목록이 같은 서비스로 다시 뜬 것을 보여준다.
- pm2 항목은 멈추지 않는다(DC-22, 2026-11-19 사용자 결정). 사용자가 pm2를 쓰지 않아 수동 검증하지 않고, 버튼이 없는 것은 합성 자료 F-06으로 확인한다.

통과: 다시 실행된 경우 그 관리 주체가 안내된다. 상태: 해당 없음(2026-11-20, 사용자 결정). 사용자 Mac의 brew 서비스에는 KeepAlive가 켜진 것이 없다
(brew nginx plist는 `RunAtLoad`만 있어 kill 하면 다시 뜨지 않았다). brew 서비스는 `brew services stop`으로 멈춰 launchd가 다시 띄우지 않는다.
"다시 실행됨" 판정은 합성 자료 테스트(`observe`·멈추기 실행기 테스트)로만 확인한다. KeepAlive 서비스를 쓰게 되면 다시 확인한다.

### M-10 — 보호 항목

대응: PR-03, DC-12·13. 예제: F-04.

- [ ] 시스템·보안 분류 항목과 launchd가 연 포트에는 멈추기 버튼이 없다.
- [ ] 다른 사용자 계정의 프로세스에는 멈추기 버튼이 없다.
- [ ] brew가 아닌 launchd 서비스(사용자 LaunchAgent)에는 멈추기 버튼이 없고 보호 이유가 보인다(DC-13).

통과: 보호 항목을 화면에서 멈출 방법이 없다. 상태: 통과(2026-11-20, 사용자 Mac, 로컬 빌드 앱). 시스템·보안 분류 항목에 멈추기 버튼이 없었다.
다른 사용자 계정의 서버는 사용자 Mac에 없어 해당 없음.

### M-11 — Docker 컨테이너

대응: RQ-04·06, DC-14. 예제: F-05.

- [ ] Docker Desktop에서 `docker run -d --name pw-test -p 18080:8080 …`처럼 포트를 연 컨테이너를 띄운다.
- [ ] 항목이 컨테이너 이름(`pw-test`)과 Docker 표시로 개발 서버에 보인다. 앱 `Docker` 항목에 그 포트가 남지 않는다.
- [ ] 멈추면 확인 칸에 `docker stop pw-test`가 보이고, 컨테이너가 멈춰(`docker ps -a`에서 Exited) 항목이 사라진다.

통과: 컨테이너가 포트 전달 프로세스가 아니라 컨테이너로 표시되고 멈춘다. 상태: 미실행.

### M-12 — hosts 도메인·nginx·인증서

대응: RQ-05, PR-05, DC-15. 예제: F-07.

- [ ] 지금 쓰는 nginx 설정(`localhost:8080`의 location 경로별 루프백 포트) 그대로 nginx를 켜면 nginx 항목에 경로별 주소가,
  넘겨받는 개발 서버 항목에 "nginx 경유"가 보인다.
- [ ] hosts에 `127.0.0.1 shop.example.test`를 넣고, mkcert 인증서로 nginx가 443에서 개발 서버(3000)로 넘기게 설정한다.
- [ ] nginx 항목과 3000번 개발 서버 항목 양쪽에 `shop.example.test`가 보인다(개발 서버에는 "nginx 경유").
- [ ] 상세에 인증서 발급자(mkcert), 만료일이 보인다.
- [ ] 만료가 가까운 인증서로 바꾸면 경고가 보인다.
- [ ] 시험 뒤 hosts와 nginx 설정을 원래대로 돌린다(앱은 이 파일들을 바꾸지 않는다).

통과: 도메인에서 실제 개발 서버까지 연결이 보인다. 상태: 일부 통과(2026-11-20, 사용자 Mac). 지금 쓰는 nginx 설정의 경로별 주소와 "nginx 경유"를 확인했다.
hosts·mkcert 도메인 항목은 아직 결과를 받지 않았다.

### M-13 — brew 설치·Spotlight·자동 실행

대응: RQ-07·08, DC-05·18·19.

- [ ] `brew install --cask superlucky84/tap/portwatch`로 설치하면 `/Applications/portwatch.app`이 생긴다.
- [ ] Spotlight에서 portwatch를 찾아 실행할 수 있다.
- [ ] 로그인 시 자동 실행을 켜고 다시 로그인하면 메뉴바에 아이콘이 생긴다.
- [ ] 새 버전이 나오면 `brew upgrade --cask portwatch`로 업데이트된다.

통과: 설치·실행·자동 실행·업데이트가 된다. 상태: 미실행. 배포 위치는 DC-18로 정했다(공개 tap 저장소의 Releases).

### M-14 — 서명 없는 앱 실행

대응: RQ-09, DC-02·21.

- [ ] 설치 직후 첫 실행에서 macOS가 막으면 README의 허용 절차(시스템 설정 → 개인정보 보호 및 보안 → 그래도 열기)로 실행된다(DC-21).
- [ ] `brew install` 끝에 cask 안내(caveats)로 같은 절차가 보인다.
- [ ] 회사 보안 에이전트가 실행을 막지 않는지 확인한다.
- [ ] 업데이트 뒤 다시 허용이 필요한지 기록한다.

통과: README 절차만으로 실행할 수 있다. 상태: 미실행.
