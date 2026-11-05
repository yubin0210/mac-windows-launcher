# macOS System Launcher — Project & Session Context Handoff

이 문서는 Antigravity / AI 어시스턴트 세션이 새 창(`mac-windows-launcher`)에서 열렸을 때 프로젝트의 전체 구현 이력, 아키텍처 및 작업 맥락을 즉시 파악하고 연속성 있게 작업을 이어가기 위한 컨텍스트 핸드오프 문서입니다.

---

## 1. 프로젝트 개요 & 현황

- **목표**: Electron이나 WebView2 같은 무거운 웹 런타임을 배제하고, Rust 2021 + Win32 + DirectComposition + Direct2D 네이티브 기술 스택으로 Windows 10/11 환경에 macOS 데스크톱 인터랙션을 1:1 완벽하게 재현하는 초경량(<35MB RAM, 0% CPU, 120Hz) 시스템 런처.
- **GitHub 저장소**: [https://github.com/yubin0210/mac-windows-launcher](https://github.com/yubin0210/mac-windows-launcher)
- **로컬 작업 경로**: `/Users/yubin/Projects/mac-windows-launcher`
- **현재 버전**: `v1.0.0` (모든 5개 마일스톤 및 인스톨러 완료, 155개 테스트 100% 통과)

---

## 2. 5개 마일스톤 완료 내역

1. **Milestone 1: REA Spec 추출 & 물리학 코어 (`crates/mac-physics-core`)**:
   - 가우시안 곡선 마우스 호버 확대/축소 곡선 (`s(x) = s_base + (s_max - s_base) * exp(-(x - x_cursor)^2 / (2*sigma^2))`).
   - 감쇠 조화 진동자(Damped Harmonic Oscillator) 스프링 바운스 물리 (`mass: 1.0, stiffness: 180.0, damping: 24.0`).
   - 스퀘어클(Squircle) 슈퍼타원(Superellipse $n=4.5$) 트레이 베지어 윤곽 계산.
   - 100% 플랫폼 독립 순수 Rust 수학 엔진.

2. **Milestone 2: App Tracker & Win32 Shell & Taskbar Guard (`crates/win-launcher`)**:
   - 실행 중인 윈도우 프로세스 추적 및 실행/포커스 핍(Indicator Pip) 표시.
   - `ProcessLauncher`: 앱 실행 및 기존 활성 창 포커스 전환.
   - `TaskbarGuard`: Windows 작업표시줄 자동 숨김(`--hide-taskbar`) 및 RAII `Drop` 안전 복원 보장.
   - Direct2D 1.1 하드웨어 가속 트레이 및 아이콘 렌더러.

3. **Milestone 3: Spotlight 검색 런처 Subsystem (`crates/win-launcher`)**:
   - `Alt + Space` 글로벌 단축키로 중앙 플로팅 스퀘어클 바 호출.
   - 인메모리 퍼지 매칭(Fuzzy Search) 인덱서 및 앱/설정 카테고리 랭킹.
   - 실시간 수학식 계산기 (`120 * 60 + 500` $\rightarrow$ `7700`).

4. **Milestone 4: Top Menu Bar & 하드웨어 상태 Subsystem (`crates/win-launcher`)**:
   - Win32 `AppBar` 상단 28px 데스크톱 작업영역 자동 예약 (`ABE_TOP`) 및 복원.
   - 실시간 배터리 잔량/충전 상태, 시스템 시계, Wi-Fi, 오디오 모니터링.
   - 활성 창 타이틀 동적 연동, Apple 사과 마크 메뉴, 스퀘어클 드롭다운 메뉴, `WM_KILLFOCUS` 자동 닫힘.

5. **Milestone 5: Stage Manager Subsystem (`crates/win-launcher`)**:
   - 앱 기반 윈도우 그룹화 및 화면 좌측 플라이아웃 선반 레이아웃.
   - Windows DWM 제로카피 하드웨어 가속 라이브 썸네일 스트리밍 (`DwmRegisterThumbnail`).
   - 다중 창 그룹 부채꼴(Fan-out) 겹침 카드 덱 및 창 개수 뱃지.
   - 원클릭 스테이지 전환 (이전 작업창 최소화 & 새 그룹 모든 창 복원/활성화).

---

## 3. 인스톨러 & CI/CD 배포 패키지 (`installer/`, `.github/workflows/`)

- **`installer/MacLauncher.iss`**: Inno Setup 전용 64-bit 설치 마법사 컴파일 스크립트 (`MacLauncher-Setup.exe`).
- **`installer/install.bat` / `install.ps1`**: 윈도우에서 더블 클릭 시 `%LOCALAPPDATA%\Programs\MacLauncher`에 자동 설치, 바로가기 생성, Windows "설치된 앱" 등록.
- **`installer/uninstall.bat` / `uninstall.ps1`**: 완전 무흔적 클린 언인스톨러.
- **`.github/workflows/release.yml`**: 버전 태그(`v1.0.0`...) 푸시 시 GitHub Windows 11 클라우드 서버에서 `win-launcher.exe` 및 `MacLauncher-Setup.exe`를 자동 빌드하여 Releases에 배포.

---

## 4. 검증 명령어 및 작업 팁

```bash
# 전체 테스트 실행 (155개 테스트)
cargo test --workspace

# Windows 크로스 컴파일 검증 (경고/에러 0개 확인)
cargo check --target x86_64-pc-windows-gnu -p win-launcher

# 런타임 시뮬레이션 데모 실행 (Dock, Spotlight, MenuBar, StageManager 전 파이프라인)
cargo run -p win-launcher
```
