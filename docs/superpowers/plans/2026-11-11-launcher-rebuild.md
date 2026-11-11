# Launcher Rebuild Implementation Plan

> **For agentic workers:** Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 승인된 설계에 따라 실제 Windows에서 사용할 수 있는 런처 구현.
**Architecture:** Rust/Win32 유지, 실행 조립과 OS 수명 관리 분리, 실제 backend만 프로덕션에 연결.
**Tech Stack:** Rust, windows 0.58, Direct2D/DirectComposition, Shell.
**Spec:** ../specs/2026-11-11-launcher-rebuild-design.md

## Global Constraints

- Windows 10/11 x64. ARM64 지원 주장 금지.
- Electron/WebView 추가 없음. 작업표시줄 기본 유지.
- macOS 테스트는 Windows UX 검증을 대체하지 않는다.
- 미완성 메뉴바/Stage Manager는 기본 실행에서 제외한다.

## Review Focus

- 로그 디렉터리 접근 실패 시 시작 실패를 사용자에게 표시.
- 중복 실행은 기존 프로세스나 창을 종료하지 않는다.
- 투명 영역이 다른 앱의 입력을 가로채지 않는다.
- 초기화/렌더링 실패를 mock 성공으로 감추지 않는다.
- DPI 변경/장치 손실은 미검증 상태를 명시한다.

## Task 1: Runtime foundation

Files: runtime.rs, diagnostics.rs, window.rs, main.rs, Cargo.toml, tests/test_diagnostics.rs.
Interfaces: diagnostics::initialize(Path) -> io::Result<PathBuf>; diagnostics::write(&str); runtime::run() -> Result<(), String>.
- [x] 로그 파일 생성/추가/회전 및 디렉터리 실패 테스트를 작성하고 실패 확인.
- [x] 로그 소유권과 panic 기록 구현, 테스트 통과 확인.
- [x] 프로덕션 실행 조립을 runtime.rs로 이동. 실제 Dock만 생성하고 초기화 실패 전파.
- [x] Per Monitor V2 DPI 설정, singleton mutex, 트레이 종료/로그 폴더 메뉴 구현.
- [x] 창별 surface와 EndDraw 오류 처리 확인. 창 입력 통과와 종료 처리 구현.
- [x] cargo test --workspace 및 cargo check --target x86_64-pc-windows-gnu -p win-launcher.
- [x] Windows backend CI smoke test 추가, 실제 표시 검증과 구분. CI 실행 결과와 실제 Windows 표시 검증은 대기 중.

## Task 2: Real Dock

Files: app_catalog.rs, shell_identity.rs, settings.rs, display.rs, dock_session.rs, dock_menu.rs, icon_cache.rs, runtime.rs, directx.rs, app_tracker.rs, process_launcher.rs, indexer.rs, window.rs.
Interfaces: AppCatalog::discover/get/pinned/spotlight_items; AppRecord::matches; SettingsStore::load/save; DockSession::refresh/visible/pin_settings/page_by; DockPlacement::fit_with_size; IconLoader::request/drain; Direct2DRenderer::set_icon/resize.
- [x] 앱 목록/설정/픽셀/배치/공유 호스트/사용자 지정 AUMID/빈 Dock 회귀 테스트를 RED→GREEN으로 실행.
- [x] 시작 메뉴 바로가기와 AppsFolder 수집, 전체 경로 및 AUMID 식별, indexer 공유.
- [x] 비동기 Shell 아이콘 추출과 제한된 채널, 실제 GPU 비트맵, 대체 글자 표시.
- [x] 앱 고정/해제/정렬, 실행 중 앱 추가/재실행, 설정 원자적 교체와 손상 원본 보존.
- [x] 모니터/크기 설정과 DPI 재배치, 확대 여유 공간과 페이지 이동.
- [x] 실제 창 선택/소유 PID 확인/실패 보고, 최대화 유지 및 최소화 복원.
- [x] native smoke 확장 및 로컬 모델 테스트/Windows 대상 컴파일.
- [ ] 실제 Windows 화면·아이콘 알파·IME 외 입력·모니터 전환·설정 복원 검증. 이 호스트에는 Windows 런타임과 링크용 MinGW compiler가 없다.

## Task 3: Spotlight

Files: spotlight_native.rs, spotlight_controller.rs, spotlight_layout.rs, spotlight_render.rs, spotlight_view.rs, spotlight_window.rs, clipboard.rs, directx.rs, runtime.rs, settings.rs, native_tray.rs, dock_menu.rs; calculator boundary in mac-physics-core/src/spotlight.rs.
Interfaces: SpotlightGeometry::fit/row_at; SpotlightView::replace_index/select_id; NativeSpotlightWindow::register_hotkey/set_results/set_row_ids; DirectCompositionPipeline::for_window; Direct2DSpotlightRenderer::with_pipeline/draw/resize; SpotlightController::frame -> Result<Option<SpotlightItem>, String>; clipboard::copy.
- [x] 공유 앱 ID·인덱스 갱신·Unicode·배치·지연 클릭 회귀 테스트 RED→GREEN.
- [x] native EDIT로 IME/붙여넣기/선택/커서 입력 위임. 입력은 4096 UTF-16 단위로 제한.
- [x] 공유 GPU 장치의 창별 target/surface, EDIT 아래 composition, DirectWrite 결과 이름/설명/ellipsis, 비동기 아이콘.
- [x] 안정적인 결과 ID 클릭, 방향키/Enter/닫기, 모니터별 DPI/검색 결과 수에 따른 창 크기.
- [x] 실제 클립보드 소유권 이전과 계산 결과 복사. 긴 입력의 계산기 재귀 깊이는 128로 제한하고 긴 평면 합계는 유지.
- [x] Alt+Space 기본값과 Ctrl+Alt+Space/Ctrl+Shift+Space 설정. 충돌 시 기존 등록 보존, 설정 저장 실패 시 복원, 트레이 Open Spotlight 제공. Ctrl+Space 자동 대체는 IME 충돌을 피하기 위해 사용하지 않음.
- [x] 실제 EDIT/DirectWrite/GPU 업로드 및 단축키 충돌 native smoke 추가. focus를 바꾸는 native smoke는 순차 실행.
- [ ] 실제 Windows에서 IME 조합/후보창/취소, paste/caret, clipboard, 결과 클릭, 반복 열기, UI 외형을 검증. 코드 검토와 cross-check는 이 검증을 대체하지 않음.

## Task 4: Menu bar

Production files: menu_bar_model.rs, menu_bar_native.rs, system_status.rs::read_menu_status, runtime.rs, settings.rs, dock_menu.rs. Legacy menu_bar_view/render/window remain demo/test modules; production does not construct their mock renderers.
Interfaces: BarModel::layout/hit; NativeMenuBar::new/frame -> Vec<MenuCommand>; read_menu_status -> BarStatus with optional measurements.
- [x] 레이아웃·실제 명령·알 수 없는 상태/기본 비활성화 테스트 RED→GREEN.
- [x] 실제 DirectComposition/DirectWrite 상단 바와 별도 네이티브 popup 메뉴. 예약 높이는 28 DIP로 고정.
- [x] 실제 시각/전원/CoreAudio 볼륨 조회, 오류 시 unknown 유지. Network는 설정 버튼이며 가짜 Wi-Fi 상태를 표시하지 않음.
- [x] 선택 모니터/DPI와 Shell ABM_NEW/QUERYPOS/SETPOS/ACTIVATE/WINDOWPOSCHANGED/REMOVE, 전체화면 Z 순서 및 Explorer 재시작 처리.
- [x] 설정 토글 저장, 초기화 실패 보고/자원 정리, native 예약/복원 smoke 작성.
- [ ] 실제 Windows 공존/전체화면/다중 모니터/Explorer 재시작/종료 후 복원 검증. 기본값 false 유지.

## Task 5: Stage Manager

Production files: stage_model.rs, stage_actions.rs, stage_events.rs, stage_native.rs, runtime.rs, settings.rs, dock_menu.rs; shared identity rule in app_catalog.rs. Legacy StageView/Tracker/Window/Render and DwmThumbnailManager remain demo/test code and are not production fallbacks.
Interfaces: ShelfModel::new/page, ShelfGeometry::fit/visible_bounds, StageChanges::switch/observed_restore/restore_all, RestoreEvents::drain, NativeStage::new/frame/animating/take_error.
- [x] 실제 포커스/앱 ID 그룹, 동일 실행파일의 AUMID 분리, 배치/축소 입력 영역 테스트 RED→GREEN.
- [x] 실제 GPU 선반/DirectWrite 제목, DwmRegisterThumbnail/QuerySourceSize/UpdateProperties와 RAII 해제. 실패는 카드별 안내와 3초 backoff, 접기 후 재시도.
- [x] 실제 창 크기와 표시/입력 영역을 함께 애니메이션하고, 완전히 열리기 전 카드 클릭 차단. 헤더 고정과 휠 페이지 전환.
- [x] 대상 활성화 성공 전 다른 창 변경 금지. 이전 실제 활성 그룹만 최소화하고 Stage의 최소화만 기록·복원. 외부 EVENT_SYSTEM_MINIMIZEEND로 기록 해제. 실패/수동 복원 회귀 테스트.
- [x] 창 선택 메뉴, PID 확인, 최대화 보존, 선택 모니터/DPI/전체화면/바탕화면 구분, 설정 토글.
- [x] 독립 검토와 Windows 대상 컴파일. 실제 DWM 등록/축소 HWND native smoke 작성.
- [ ] 실제 Windows 썸네일/comp 레이어 순서/IME 외 입력/창 상태/외부 이벤트/다중 모니터 검증. 기본 false 유지. 최소화/보호된 창의 DWM 한계는 별도 확인.

## Task 6: Release

Files: scripts/package-windows.ps1, test-installer.ps1, measure-process.ps1; installer/*; .github/workflows/*; main.rs/runtime.rs/shutdown.rs; test_release_cli.rs; README/Windows 검증 문서.
Interfaces: Cargo version -> binary/Setup/manifest; same-path shutdown -> WM_CLOSE + bounded wait; smoke startup-ready/startup-smoke reports -> installed payload lifecycle test.
- [x] CLI --version/비 Windows --quit 회귀 테스트 RED→GREEN. Cargo 1.1.0-alpha.1을 버전 원천으로 사용.
- [x] Inno 설치/제거로 통일, 자동 시작 기본 해제, 설정·로그/사용자 추가 파일 보존, 동일 경로만 정상 종료 요청.
- [x] release workspace 테스트·선택 native smoke·해시·manifest·로그를 만드는 단일 패키징 스크립트.
- [x] disposable profile 새 설치/실행 중 재설치/제거, 설정 보존, 자동 시작 on/off와 구형 등록 fixture 검증 스크립트.
- [x] 실제 프로세스 CPU/working set 조건별 표본 도구. 프레임 시간/GPU 메모리/실제 동작 확인은 자동 측정하지 않음을 명시.
- [x] 수동 preview workflow, contents:read, 성공 패키지/실패 로그 artifact. 태그 자동 정식 발행 제거.
- [x] Dock DComp host의 초기화 없는 layered 스타일 제거 및 native 회귀 assertion 추가. Windows 실행은 대기.
- [x] Explorer TaskbarCreated를 Dock 이벤트로 전달하고 트레이 재등록/실패 보고, native 알림 주입 검사 작성. 실제 재시작 확인 대기.
- [x] Windows CI(Server 2025/Hyper-V)에서 PowerShell AST/Inno 컴파일/실제 설치·실행·재설치·제거 통과. 204 workspace +8 native, 실행43045554382. Windows10/11 실기 외형/입력과 구분.
- [ ] 실제 이전 버전 업그레이드/자동 시작 후 재부팅, 애니메이션·검색 CPU/RAM/프레임 측정과 UI 검증 기록.
- [ ] 최종 Windows 실기 검증 전 공개 배포하지 않는다.
