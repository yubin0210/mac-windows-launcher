# macOS System Launcher for Windows — Installer Package

이 디렉토리는 Windows 10/11 환경에서 **macOS System Launcher (`win-launcher.exe`)**를 클릭 한 번으로 설치하고 바로 사용할 수 있도록 돕는 설치 파일 패키지입니다.

---

## 1. 구성 파일 목록

- **`install.bat` / `install.ps1`**:
  - 추가 소프트웨어 설치 없이 Windows 기본 PowerShell만으로 즉시 설치를 완료하는 원클릭 설치기입니다.
  - `%LOCALAPPDATA%\Programs\MacLauncher\` 경로에 무권한(Non-admin)으로 안전하게 설치됩니다.
  - 바탕화면 바로가기, 시작 메뉴 바로가기, 부팅 시 자동 시작 등록, Windows "설치된 앱" 등록을 자동으로 처리합니다.
- **`uninstall.bat` / `uninstall.ps1`**:
  - 생성된 바로가기, 시작프로그램 등록, 설정 키, 복사된 바이너리를 찌꺼기 없이 깨끗하게 삭제하는 언인스톨러입니다.
- **`MacLauncher.iss`**:
  - 전 세계 표준 Windows 설치 마법사 소프트웨어인 **Inno Setup** 전용 컴파일 스크립트입니다.
  - `ISCC.exe MacLauncher.iss`를 실행하면 단일 설치 마법사 실행 파일인 **`MacLauncher-Setup.exe`**가 빌드됩니다.
- **`.github/workflows/build-maclauncher-installer.yml`**:
  - GitHub Actions를 통해 Windows 서버에서 `win-launcher.exe`와 `MacLauncher-Setup.exe`를 자동 빌드하여 다운로드 링크를 제공하는 워크플로우입니다.

---

## 2. 사용 방법

### 방법 A: `install.bat` 더블클릭 (가장 간편한 방법)
1. `win-launcher.exe` 파일과 `install.bat`, `install.ps1` 파일을 같은 폴더에 둡니다.
2. `install.bat`을 더블클릭합니다.
3. 자동으로 `%LOCALAPPDATA%\Programs\MacLauncher`에 설치되고 바탕화면에 바로가기가 생성되며 즉시 실행됩니다.

### 방법 B: `MacLauncher-Setup.exe` 마법사 설치 파일 생성
Windows 환경에서 Inno Setup이 설치되어 있다면:
```powershell
iscc installer\MacLauncher.iss
```
`target\installer\MacLauncher-Setup.exe`가 생성되며, 일반 상용 프로그램처럼 "다음 > 다음 > 완료" 버튼을 눌러 설치할 수 있습니다.

### 방법 C: GitHub Actions를 통한 자동 생성
코드를 GitHub에 Push하면 `.github/workflows/build-maclauncher-installer.yml`이 자동으로 실행되어 GitHub 저장소의 **Actions $\rightarrow$ Artifacts**에서 `MacLauncher-Setup.exe`를 즉시 다운로드할 수 있습니다.
