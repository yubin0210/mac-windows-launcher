# macOS-style Windows Launcher

Rust + Win32 + Direct2D/DirectComposition 기반 Windows 런처입니다. 현재 기존 프로토타입을 재구현하고 있습니다. 정식 완성 제품으로 간주하지 마세요.

## 현재 실행 상태

- Windows 프로덕션 진입점: `crates/win-launcher/src/runtime.rs`.
- Dock: 시작 메뉴 바로가기와 Windows AppsFolder에서 앱 목록을 읽고 Shell 아이콘을 비동기로 추출해 표시합니다. 아이콘 추출 실패 시 앱 이름의 앞 글자를 표시합니다.
- 고정 앱과 실행 중 앱을 함께 표시하며, 실행 파일 경로와 AUMID로 앱을 구분합니다. 여러 창이 있으면 창 선택 메뉴를 제공합니다.
- Dock 우클릭: 앱 고정/해제, 좌우 순서 변경, 앱 추가, 화면 선택, 크기 변경, 페이지 이동. 마우스 휠로도 페이지를 바꿉니다.
- 고정 목록·앱 실행 정보·대상 화면·크기는 `%LOCALAPPDATA%\MacLauncher\settings.json`에 저장합니다. 손상된 파일은 원본을 백업한 후 초기화하며, 더 높은 버전의 설정 파일은 덮어쓰지 않습니다.
- 사용자가 Dock를 비운 상태를 재시작 후에도 유지합니다. 실행 중 앱은 별도로 나타나며 우클릭 **Keep in Dock**으로 고정할 수 있습니다.
- Spotlight: **Alt + Space**로 검색창을 열고 앱/설정/계산식을 검색합니다. 방향키·Enter 또는 결과 클릭으로 실행하며, 계산 결과는 Enter로 복사합니다. Escape나 외부 클릭으로 닫습니다.
- 단축키 변경: Dock 우클릭 → **Spotlight shortcut**. 충돌 시 오류를 알리고 기존 단축키를 유지합니다. 트레이 **Open Spotlight**로도 열 수 있습니다.
- 검색 입력은 Windows EDIT가 IME·커서·선택·붙여넣기를 처리합니다. 검색 결과는 실제 DirectWrite/Direct2D로 그립니다. 실제 한글 IME 동작은 Windows 검증 대기 상태입니다.
- 메뉴바: Dock 우클릭 → **Show menu bar**로 켭니다. 선택한 화면 상단에 작업영역을 예약하며 실제 활성 앱 이름·시각·조회 가능한 배터리/볼륨을 표시합니다. 메뉴/상태 항목은 Windows 설정, 작업 관리자, Spotlight, 로그 열기로 연결됩니다.
- Stage Manager: Dock 우클릭 → **Enable Stage Manager**로 켭니다. 같은 화면의 왼쪽 가장자리 핸들에 마우스를 올리면 다른 앱 그룹의 썸네일 선반이 펼쳐집니다. 헤더로 펼침을 고정하고 휠로 페이지를 바꿉니다. 카드 클릭으로 그룹을 전환하며 우클릭으로 창을 선택합니다.
- 메뉴바와 Stage Manager는 실제 Windows 검증 전까지 기본 꺼짐입니다. Stage를 끄면 Stage가 최소화했던 창만 복원을 시도하며, 외부에서 복원한 창은 관리 기록에서 제외합니다.
- 트레이 아이콘 우클릭: **Open Spotlight**, **Open logs**, **Exit MacLauncher**.
- 로그: `%LOCALAPPDATA%\MacLauncher\logs\launcher.log`. 시작 시 2MiB 이상이면 이전 로그로 회전합니다.
- 중복 실행은 추가 인스턴스를 생성하지 않습니다. Windows 작업표시줄은 유지합니다. 이전 `--hide-taskbar` 옵션은 재구현 중 지원하지 않습니다.
- DPI awareness, 화면 선택, 배율/작업영역 변경 시 재배치를 구현했습니다. 실제 다중 모니터 전환과 device lost 복구 검증은 남아 있습니다.

기존 릴리스와 이전 문서의 ‘5개 마일스톤 완료’, ‘정확한 macOS 재현’, ‘35MB/0% CPU/120Hz’ 표기는 현재 검증된 품질이나 성능을 뜻하지 않습니다.

## 빌드와 검증

Windows 10/11 x64, Rust stable, Visual Studio C++ Build Tools가 필요합니다.

```sh
cargo test --workspace --locked
cargo build --release --locked -p win-launcher
```

실제 Windows 데스크톱의 렌더러 초기화 검증:

```sh
cargo test --locked -p win-launcher --test test_native_backend -- --ignored --nocapture --test-threads=1
```

이 smoke test는 HWND, GPU 장치, 비트맵 업로드와 그리기, Shell 앱/아이콘 조회, 창 상태 보존, native EDIT·DirectWrite, 단축키 충돌을 실행합니다. 화면 외형과 사용자 입력 검증은 별도로 필요합니다. macOS의 `cargo run -p win-launcher`는 시뮬레이션 데모만 실행합니다.

macOS에서 Windows 코드 컴파일 확인:

```sh
cargo check --locked --target x86_64-pc-windows-gnu -p win-launcher --all-targets
```

## 작업 문서

- [승인된 재구현 설계](docs/superpowers/specs/2026-11-11-launcher-rebuild-design.md)
- [작업 계획](docs/superpowers/plans/2026-11-11-launcher-rebuild.md)
- [Windows 검증 기록](docs/windows-validation.md)

## 프리뷰 설치 패키지

버전은 `1.1.0-alpha.1`입니다. Windows에서 PowerShell 7과 Inno Setup 6을 준비한 뒤 아래 명령으로 테스트한 release 실행 파일을 패키징합니다.

```powershell
.\scripts\package-windows.ps1 -RunNativeSmoke
# 기존 설치/데이터가 없는 일회용 Windows 계정에서만 실행
.\scripts\test-installer.ps1
```

설치·제거는 Inno Setup으로 통일했습니다. 자동 시작은 선택 사항이며 설정·로그는 제거 후에도 보존합니다. 실행 중 업데이트·제거는 동일 경로의 런처에 정상 종료를 요청합니다. 상세 내용은 [설치 패키지 안내](installer/README.md)를 참고하세요.

수동 **Windows Preview Package** 워크플로는 빌드/설치 검증 후 해시·manifest·로그를 artifact로 보관합니다. 태그만으로 정식 릴리스를 발행하지 않습니다. 로컬 Mac의 컴파일 검사에 이어 Windows CI에서 MSVC release 테스트 204개, native API 검사 8개, 실제 설치·실행 중 재설치·제거 검사를 통과했습니다. [최초 성공 실행](https://github.com/yubin0210/mac-windows-launcher/actions/runs/43045554382)은 Windows Server 2025/Hyper-V 환경이며 Windows 10/11의 실제 화면·한글 IME·다중 모니터 검증은 남아 있습니다.

## 현재 한계

실제 Windows 표시/입력 검증을 아직 수행하지 않았습니다. 앱이 선언한 식별자와 실행 파일 정보를 확인할 수 없으면 실행 상태를 추측하지 않습니다. 인수가 있는 바로가기는 원래 바로가기를 실행하며, 명시적 앱 ID가 있을 때 기존 창과 연결합니다. 새로 설치한 앱 목록은 재시작 시 반영합니다. 장치 손실 복구는 후속 작업이며 설치 패키지는 Windows CI 검사를 통과했으며 Windows 10/11 실사용 검증은 남아 있습니다. Stage Manager는 실제 DWM 표시·입력 영역·외부 복원 이벤트·창 상태 보존 검증이 남아 있습니다. 메뉴바는 Windows AppBar 공존·전체화면·화면 이동·종료 후 복원 검증이 남아 있습니다. Spotlight는 실제 Windows에서 입력·외형·클립보드 검증이 남아 있습니다. 검색 입력은 최대 4096 UTF-16 단위, 계산식의 재귀 중첩은 최대 128단계입니다.

메뉴바의 Network는 네트워크 설정 바로가기입니다. Wi-Fi 연결 상태를 추측해 표시하지 않습니다. 배터리/오디오 조회가 실패하면 해당 수치는 표시하지 않습니다. Windows 앱의 File/Edit 메뉴를 가져오는 기능은 제공하지 않습니다.

Stage 미리보기는 DWM이 제공하는 내용입니다. 최소화된 창은 마지막 화면으로 멈출 수 있으며 보호된 창은 미리보기를 제공하지 않을 수 있습니다. 조회 실패 시 앱 이름을 유지하고 오류 안내를 표시하며 재시도합니다. 전체화면 앱이 해당 화면을 덮으면 선반을 숨깁니다.
