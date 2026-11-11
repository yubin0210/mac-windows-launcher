# 런처 재구현 작업 맥락

2026-11-11: 사용자 설계 승인 후 feat/launcher-rebuild 브랜치에서 구현 시작.

원래 문서의 전체 완료/성능/정확도 주장은 검증되지 않았다. 실제 실행에서 Spotlight, 메뉴바, Stage Manager 렌더러가 mock이었다. Dock 아이콘도 사각형이었다.

현재 프로덕션 진입점은 runtime.rs. macOS 데모는 demo.rs로 분리했다. 미완성 기능은 기본 실행에서 제외했다. 로그, 중복 실행 방지, DPI awareness, 트레이 종료 및 로그 접근, Dock 영역 밖 입력 통과와 유휴 redraw 억제를 추가했다.

계획: docs/superpowers/plans/2026-11-11-launcher-rebuild.md
설계: docs/superpowers/specs/2026-11-11-launcher-rebuild-design.md
검증: docs/windows-validation.md

Windows cross-check는 실제 Windows 실행 검증이 아니다. 실제 앱 아이콘/앱 모델/설정/Spotlight/메뉴바/Stage Manager, DPI 변경/장치 복구와 설치 검증은 남아 있다. 코드를 다시 읽고 작업 기록에 따라 이어간다.

2단계 코드 추가: app_catalog.rs가 시작 메뉴/AppsFolder 데이터를 소유한다. AppRecord의 AUMID 또는 실행 파일 경로가 창 매칭의 기준이다. shell_identity.rs가 Shell 속성과 프로세스 AUMID를 읽는다. AppTracker는 전체 실행 경로와 AUMID를 보존한다.

settings.rs는 버전/핀/사용자 앱/모니터/아이콘 크기를 원자적으로 저장하고 손상 파일을 백업한다. dock_session.rs가 고정 앱과 실행 중 앱을 합치고 페이지를 계산한다. dock_menu.rs가 네이티브 설정/창 선택 메뉴를 제공한다. display.rs가 화면과 배율 배치를 소유하며, icon_cache.rs의 제한된 채널을 사용하는 worker가 CPU 픽셀만 UI로 전달한다. directx.rs가 실제 GPU 비트맵과 대체 글자를 그린다.

2단계의 데이터/배치 테스트 및 Windows 대상 컴파일은 로컬에서 수행했다. native smoke에 Shell 아이콘/실제 GPU 업로드/최대화 유지 검사를 추가했지만 실제 Windows에서는 아직 실행하지 않았다. Windows로 링크할 MinGW compiler도 이 호스트에 없어 새 exe는 만들지 않았다. 다음 구현 대상은 실제 Spotlight다. 기존 테스트 중 가짜 HWND/PID로 앱을 열던 시뮬레이션 테스트는 macOS 등 비 Windows에서만 실행하도록 구분했다.

3단계 코드 추가: spotlight_native.rs의 native EDIT가 입력/IME/선택/paste/caret를 소유하며, 부모 HWND는 이벤트/배치/포커스/단축키만 관리한다. 기존 spotlight_window.rs Windows 구현을 교체하고 mock은 명시적 개발/테스트 경로에 보존했다.

spotlight_controller.rs가 공유 앱 인덱스, 결과 ID 선택, 비동기 아이콘 및 실제 surface를 조립한다. directx::for_window는 GPU 장치를 공유하고 창별 target을 만들며 Spotlight target은 native EDIT 아래 레이어에 놓인다. spotlight_render.rs는 실제 DirectWrite 결과와 아이콘을 그린다. runtime.rs가 실제 실행/계산 결과 복사와 닫기를 연결한다.

단축키는 Settings의 SpotlightHotkey enum이며 AltSpace 기본, CtrlAltSpace/CtrlShiftSpace 선택 가능. register-before-unregister로 충돌 시 기존 등록을 보존한다. 시작 시 충돌해도 트레이 Open Spotlight를 사용할 수 있다. 입력 4096 UTF-16 단위, 계산기 재귀 128단계 제한. math parser의 괄호/단항/거듭제곱 재귀 테스트를 추가했다.

순수 모델/Windows 컴파일 검사 및 독립 코드 검토를 수행했다. 실제 Windows IME/표시/Clipboard 확인은 여전히 미검증이며, native smoke 추가를 실제 실행 성공으로 해석하지 않는다. 다음 단계는 메뉴바다.

4단계 코드: menu_bar_model.rs와 menu_bar_native.rs가 프로덕션 메뉴바를 담당한다. 기존 menu_bar_view/render/window는 레거시 demo/mock이며 새 런타임에서 사용하지 않는다. 메뉴바는 기본 false이며 Dock 메뉴에서 켠다. 실제 DirectWrite와 공유 GPU surface, 독립 native popup을 사용한다. system_status::read_menu_status는 실제 시계/배터리/CoreAudio만 읽고 실패는 Option::None으로 남긴다. Network는 설정 버튼이다.

Shell AppBar NEW/QUERYPOS/SETPOS/ACTIVATE/WINDOWPOSCHANGED/REMOVE 및 위치/전체화면/Explorer 재시작 콜백을 연결했다. 단일 reserved 플래그가 등록 소유권을 관리한다. 위치 이벤트나 대상 화면/DPI 변경 때만 재협상하고 dropdown으로 예약 높이를 늘리지 않는다. Windows 예약/복원 native smoke를 작성했으나 이 호스트에서는 실행하지 않았다. 다음 구현 대상은 Stage Manager다.

5단계 코드: stage_model.rs(실제 앱 ID/포커스 그룹·페이지·배치), stage_actions.rs(활성화 우선/최소화 소유권), stage_events.rs(외부 복원 이벤트), stage_native.rs(실제 GPU 선반/DWM/PID/애니메이션)로 구성한다. 레거시 stage_*/DwmThumbnailManager mock 경로는 새 프로덕션 선반에 연결하지 않는다.

Stage 기본 false. Dock 메뉴로 활성화한 뒤 가장자리 hover, 헤더 pin, 휠 페이지, 카드/우클릭 창 선택을 사용한다. 전체화면에선 숨기되 Progman/WorkerW 바탕화면은 제외한다. DWM 실패는 3초 backoff/다시 펼침 재시도다. AppRecord의 exe fallback은 양쪽 AUMID가 모두 없을 때만 허용하여 generic host가 PWA를 합치지 않게 했다.

Stage가 직접 최소화한 HWND/PID만 기록한다. 외부 MINIMIZEEND 이벤트는 값만 담는 thread-local registry로 수집하고 frame/Drop 전에 소유권에서 제외한다. Drop는 남아 있는 동일 PID의 최소화 창만 복원을 시도하고 실패는 로그에 남긴다. Native 이벤트/이미지/comp 레이어와 실제 복원은 Windows 검증 대기. 다음 단계는 설치/배포 및 실제 Windows 검증이다.


6단계 코드: Cargo의 win-launcher 버전 1.1.0-alpha.1을 binary/Setup/manifest 원천으로 사용한다. main은 --version, Windows --quit, --smoke-test 및 --smoke-test --wait-for-quit을 지원한다. shutdown.rs는 실행 이미지 경로를 확인하고 WM_CLOSE 후 10초 기다린다. runtime smoke는 첫 composed frame ready PID와 종료 component report를 파일로 기록하며 visual_ime_verified:false를 유지한다. 일반 실행은 smoke 파일을 쓰지 않는다.

설치는 Inno 단일 lifecycle이며 PS/bat는 wrapper다. broad recursive delete/이름 기반 강제 종료를 제거했고 설정·로그를 보존한다. 자동 시작 기본 해제. 이전 PS 등록은 경로/제거 명령이 모두 일치하는 경우만 정리한다. scripts/package-windows.ps1은 PowerShell7/Windows/MSVC/Inno6로 release workspace 테스트·선택 native smoke·동일 exe/설치기·해시·manifest를 생성한다. scripts/test-installer.ps1은 기존 데이터가 없는 Windows 계정만 허용하고 실제 smoke 프로세스를 기동해 실행 중 재설치/제거·설정 보존·자동 시작 선택·legacy fixture를 검사한다. 종료 타임아웃의 강제 종료는 테스트가 소유한 disposable process에만 적용하며 제품 설치기에는 없다.

scripts/measure-process.ps1은 명시 PID/실행 경로의 CPU/working set만 측정하고 프레임 시간/GPU 메모리와 실제 동작 여부를 미측정으로 둔다. 수동 preview workflow는 테스트 성공 패키지와 로그를 artifact로 보관하며 공개 Release 자동 발행은 제거했다. 실제 이전 공개 버전 업그레이드/재부팅/Windows UI/성능 수치는 아직 없다. Windows/PowerShell/Inno가 없는 Mac이라 실제 패키지도 아직 생성하지 않았다.

최종 검토에서 Dock의 WS_EX_LAYERED와 NOREDIRECTIONBITMAP 조합에 layered 초기화가 없던 문제를 발견했다. DComp alpha를 유지하며 layered 플래그를 제거했고 native style assertion을 추가했다. 이 회귀 검사는 Windows 실행 대기이며 cross-check만으로 표시 성공을 주장하지 않는다.

최종 검토의 두 번째 수정: Dock가 TaskbarCreated 등록 메시지를 WindowEvent::ShellRestarted로 전달하고 런타임이 TrayIcon::readd를 호출한다. Explorer가 재시작돼도 트레이를 다시 등록하며 실패를 보고한다. native smoke는 test-owned 아이콘 제거 후 알림 주입/재등록 경로를 검사한다. 실제 Explorer 재시작 검증은 대기다.

6단계 로컬 최종 검증: cargo test --workspace --locked 211 passed/0 failed; cargo check --target x86_64-pc-windows-gnu -p win-launcher --all-targets --locked exit0/no warnings; git diff --check exit0; Ruby YAML parser로 두 workflow 구문 확인. PowerShell AST/Inno/native Windows 실행은 미실행. 변경은 feat/launcher-rebuild의 미커밋 상태이며 공개 배포/원격 push는 하지 않았다.


2026-11-12 KST 원격 검증: 사용자가 커밋·푸시·Actions 실행을 명시 승인. feat/launcher-rebuild에 1db8766(재구현), a4135bb(실제 HWND 초기 Paint를 queue 주입 전에 drain), 5e9bd9d(GetLongPathNameW로 8.3/긴 경로 비교 및 안전한 바로가기/파일 보존) 푸시. 세 번째 실행43045554382 성공, Windows Server2025/Hyper-V에서 MSVC workspace204 + native8 통과, Inno6.7.1 컴파일과 실제 설치/기동/실행 중 재설치/제거·설정 보존·자동 시작 on/off·legacy fixture·타 앱 바로가기·사용자 추가 파일 보존 통과. 다운로드한 exe/Setup은 manifest SHA256 재확인했다. 첫 실패는 startup Paint test, 다음 실패는 short/long startup shortcut path mismatch였고 같은 검사로 수정 후 통과했다.

실제 Windows10/11 외형·IME·모니터·Explorer restart·재부팅·이전 공개 바이너리 업그레이드·프레임 시간은 여전히 미검증이다. CI server/API 성공을 실사용 완성으로 주장하지 않는다. hosted idle fixture 표본(빈 핀/menu+stage on/30.16s): CPU단일코어0.207%, 전체머신0.052%, working set평균46.1MiB. 성능 보장이나 FPS가 아니다. 정식Release/merge는 하지 않았다. 이 아래 최신 원격 기록이 이전 미실행/미커밋 상태 설명보다 우선한다.
