# MacLauncher 프리뷰 설치 패키지

현재 버전은 `1.1.0-alpha.1`이며 Windows 10/11 x64 대상입니다. 실제 화면·한글 IME·다중 모니터 사용성 검증은 남아 있습니다.

`MacLauncher-Setup.exe`를 실행해 설치합니다. 기본 위치는 `%LOCALAPPDATA%\Programs\MacLauncher`이며 관리자 권한이 필요하지 않습니다. 자동 시작과 바탕화면 바로가기는 기본 해제이며 설치 화면에서 선택합니다. 기존 Inno 설치의 작업 선택은 유지될 수 있습니다. 시작 메뉴 바로가기는 기본 생성합니다.

`install.bat` / `install.ps1`은 같은 폴더의 Setup을 실행하는 보조 파일입니다. 별도 다운로드·빌드·프로세스 강제 종료를 하지 않습니다.

```powershell
# Windows 기본 PowerShell에서도 설치 보조 파일은 사용 가능
.\install.ps1 -AutoStart
.\install.ps1 -Silent -NoLaunch
.\uninstall.ps1 -Silent
```

제거는 Windows의 설치된 앱 또는 `uninstall.bat`으로 실행합니다. Inno가 추적하는 파일과 바로가기를 제거하며 `%LOCALAPPDATA%\MacLauncher`의 설정·로그는 보존합니다. 설치 폴더에 사용자가 따로 넣은 파일을 재귀 삭제하지 않습니다.

설치·제거 시 동일 설치 경로의 실행 중 Dock 창에 정상 종료를 요청하고 최대 10초 기다립니다. 종료되지 않으면 작업을 중단합니다. 다른 경로의 런처를 이름만으로 종료하지 않습니다. 이전 PowerShell 설치의 등록은 설치 경로와 제거 명령이 모두 일치할 때만 정리합니다. 이전 버전 실행 파일을 사용한 실제 업그레이드는 별도 검증이 필요합니다.

## 패키지 빌드

Windows x64, Rust stable/MSVC C++ 도구, Inno Setup 6, PowerShell 7이 필요합니다. 저장소 루트에서 실행합니다.

```powershell
.\scripts\package-windows.ps1 -RunNativeSmoke -ExpectedTag v1.1.0-alpha.1
# 기존 런처 데이터가 전혀 없는 일회용 Windows 사용자 계정에서만 실행
.\scripts\test-installer.ps1
```

Cargo의 버전을 읽어 바이너리·설치 표시 버전·PE 숫자 버전을 생성합니다. workspace 테스트와 같은 release 경로의 실행 파일을 설치기에 넣고 해시를 검사합니다. `-RunNativeSmoke`는 실제 API smoke를 순차 실행합니다. 생략 시 manifest에 `not-run`으로 기록합니다.

출력은 `target\installer`입니다.

- `MacLauncher-Setup.exe`, `win-launcher.exe`, 설치·제거 보조 파일
- `build-manifest.json`: 소스 SHA/dirty 상태, 도구·Windows 버전, 테스트 상태, 파일 SHA-256
- `SHA256SUMS.txt`: 실행 파일과 설치기 해시

검증 로그는 `target\validation-logs`입니다. 설치 검사는 새 설치, 설정 보존, 실행 중 같은 버전 재설치, 자동 시작 선택/해제, 실행 중 제거와 이전 등록 fixture를 검사합니다. 메뉴바·Stage를 켠 런처를 실제 기동하고 첫 composed frame 및 종료 보고서를 확인하며 유휴 CPU/working set을 30초 표본 측정합니다. 기존 설치·설정·실행 중 프로세스가 있으면 시작을 거부합니다. 성공 후에도 검사 계정에 남은 로그·설정은 검토를 위해 보존하므로 다음 검사는 새 계정에서 실행합니다.

`.github/workflows/release.yml`의 **Windows Preview Package**는 수동 실행만 지원하고 검증된 패키지와 로그를 Actions artifact로 올립니다. 정식 GitHub Release를 자동 발행하지 않습니다. 실제 실행 결과는 해당 artifact의 build-manifest.json과 검증 로그에서 확인합니다. 2026-11-12 KST 첫 Windows CI 성공 기록은 [검증 문서](../docs/windows-validation.md)에 있습니다. 생성할 패키지는 서명되지 않은 프리뷰입니다.

성능의 애니메이션·검색 조건, 프레임 시간, 실제 한글 IME·다중 모니터·재부팅 검증은 [Windows 검증 기록](../docs/windows-validation.md)에 별도로 기록합니다. API smoke 통과와 CPU 표본만으로 외형·FPS를 보증하지 않습니다.

Inno 구현 근거: [DLL 호출](https://jrsoftware.org/ishelp/topic_scriptdll.htm), [설치 이벤트](https://jrsoftware.org/ishelp/topic_scriptevents.htm), [AppVersion](https://jrsoftware.org/ishelp/topic_setup_appversion.htm), [PE 버전](https://jrsoftware.org/ishelp/topic_setup_versioninfoversion.htm), [자동 앱 종료 옵션](https://jrsoftware.org/ishelp/topic_setup_closeapplications.htm).
