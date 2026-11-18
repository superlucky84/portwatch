# portwatch

로컬 개발 서버를 한곳에서 보고 멈추는 macOS 메뉴바 앱입니다.

> 계획 단계입니다. 아직 설치하거나 실행할 수 있는 코드가 없습니다.

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

## 계획한 설치 방법

Homebrew cask로 설치할 예정입니다. 배포 위치는 아직 정하지 않았습니다([DC-18](./docs/DESIGN.md#결정-체크리스트)).
Apple 서명 없이 배포하므로 첫 실행 때 macOS의 허용 절차가 필요합니다. 절차는 배포 단계에서 이곳에 적습니다.
