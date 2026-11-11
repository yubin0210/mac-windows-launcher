# Windows 검증 기록

2026-11-11 구현 시작, 2026-11-12 KST Windows CI 실행 결과 추가. 로컬 호스트는 macOS ARM64다. Windows CI의 API·설치 검사와 실제 Windows 10/11 화면·입력 검증을 구분한다.

## 자동 확인

- 로컬 순수 모델 및 mock 테스트: cargo test --workspace.
- Windows 전용 코드 컴파일: cargo check --target x86_64-pc-windows-gnu -p win-launcher --all-targets.
- 실제 backend smoke: Windows CI에서 8개 통과. 환경과 실행 링크는 아래 2026-11-12 기록 참고.

## 실제 Windows에서 확인할 사항

빌드 SHA, Windows 버전, GPU, 화면 해상도/배율을 결과와 함께 기록한다.

- [ ] 시작 시 Dock와 트레이가 보이고 콘솔은 열리지 않는다.
- [ ] 트레이 Open logs로 로그 위치가 열린다.
- [ ] 두 번째 실행은 기존 실행을 방해하지 않고 종료한다.
- [ ] 트레이 Exit로 Dock/트레이가 제거된다.
- [ ] Explorer 재시작 후 트레이가 다시 등록되고 Open Spotlight/Open logs/Exit가 동작한다. native smoke는 알림 주입/재등록까지이며 실제 Explorer 재시작은 별도 확인한다.
- [ ] Dock 주위 빈 영역에서 다른 앱을 클릭할 수 있다.
- [ ] 앱 클릭/hover/bounce가 동작하며 유휴 상태에서 불필요한 redraw가 멈춘다.
- [ ] 그래픽 초기화 실패 시 오류 대화상자와 로그가 남는다.
- [ ] Windows 작업표시줄이 실행/종료/충돌 후 유지된다.
- [ ] 100/150/200% 배율에서 표시 상태를 기록한다. 동적 DPI/배치 코드는 구현했으나 실제 전환은 미검증이다.

## 미완성 범위

device lost 복구와 실제 Windows 10/11 사용성 검증이 남아 있다. Dock/Spotlight/메뉴바/Stage Manager의 API smoke 및 설치·재설치·제거는 Windows CI에서 실행했고 유휴 CPU/working set 표본이 있다. 외형·IME·실제 모니터 전환·재부팅·이전 공개 버전 업그레이드·애니메이션/검색 프레임 측정은 미검증이다.

## 2단계 실제 Windows 확인 항목

- [ ] 시작 메뉴와 Store 앱이 실제 이름/아이콘으로 보인다. 대체 글자만 나오는 앱은 로그와 함께 기록한다.
- [ ] Dock 우클릭으로 추가/제거/순서/크기를 바꾸고 재시작 후 복원된다.
- [ ] Dock를 비운 뒤에도 우클릭 안내가 보이며, 재시작 후 빈 설정이 유지된다.
- [ ] 실행 중인 비고정 앱을 Keep in Dock으로 고정하고 종료/재시작 후 다시 실행할 수 있다.
- [ ] 계산기 등 Store 앱의 고정 아이콘과 실행 중 앱이 중복되지 않는다.
- [ ] 포터블 앱의 사용자 지정 AUMID를 AppsFolder 실행 경로로 오인하지 않는다.
- [ ] 여러 창이 열렸을 때 선택 메뉴가 나오고, 최대화 창 활성화 시 크기가 유지된다.
- [ ] 최소화 창은 복원되고 종료된 창이나 바뀐 HWND는 성공으로 보고하지 않는다.
- [ ] 같은 이름의 다른 실행 파일 경로를 서로 다른 앱으로 처리한다.
- [ ] 화면 메뉴로 보조 모니터를 고르고 재시작/분리/배율 변경 시 표시 상태를 확인한다.
- [ ] 100/150/200%와 작은 해상도에서 확대·bounce·페이지 전환이 잘리지 않는다.
- [ ] 설정 파일을 손상시켰을 때 원본 백업이 남고, 저장 실패 시 UI가 저장된 것처럼 바뀌지 않는다.

실제 API smoke는 `cargo test -p win-launcher --test test_native_backend -- --ignored --nocapture --test-threads=1`로 실행한다. 일반 테스트가 Windows에서 임의 앱을 실행하거나 가짜 HWND를 활성화하지 않도록 시뮬레이션과 분리했다.

아이콘 추출을 UI 스레드에서 분리한 근거: [Microsoft IShellItemImageFactory::GetImage 문서](https://learn.microsoft.com/windows/win32/api/shobjidl_core/nf-shobjidl_core-ishellitemimagefactory-getimage).

## 3단계 Spotlight 실제 Windows 확인 항목

- [ ] Alt+Space로 실제 검색창이 표시되고 검색어와 결과가 함께 보인다. native EDIT가 composition 뒤에 가려지지 않는다.
- [ ] 한글 자모 조합·후보창 선택·Backspace·Enter 조합 확정이 정상이며, 조합 확정만으로 앱이 실행되지 않는다.
- [ ] IME 조합 중 Escape는 조합을 취소하고, 조합 종료 후 Escape는 검색창을 닫는다.
- [ ] 조합 중 다른 앱을 클릭하면 조합 종료와 함께 검색창이 닫힌다. IME/소유 오류 대화상자는 검색창을 잘못 닫지 않는다.
- [ ] 이모지·한글 paste, Ctrl+A/C/V/X, 커서 이동·선택·수평 스크롤과 긴 검색어를 확인한다.
- [ ] 검색어를 빠르게 바꾸고 결과를 클릭해도 다른 앱이 실행되지 않는다.
- [ ] 방향키/Enter/마우스 결과 선택, 기존 앱 창 전환, 실행 실패 후 오류 표시를 확인한다.
- [ ] `120 * 60 + 500` → Enter 후 클립보드에 `7700`이 들어간다. Clipboard가 바쁘면 오류를 표시하고 검색 내용을 유지한다.
- [ ] 빈 결과/긴 이름/긴 설명/선택 강조/아이콘/글자 잘림을 확인한다.
- [ ] Alt+Space 충돌 시 트레이 Open Spotlight가 작동하고, 다른 단축키 선택 및 재시작 후 복원이 된다.
- [ ] 검색창은 호출 당시 활성 화면에 열리며, 배율·화면 변경 후 입력 영역과 결과 클릭 좌표가 맞는다.
- [ ] 반복 열기/닫기 후 이전 검색어·IME 조합·선택 상태가 남지 않고, Escape 시 이전 앱 포커스로 돌아간다.

native smoke에 EDIT Unicode 조회, 실제 DirectWrite 결과/아이콘 그리기와 전역 단축키 충돌 검사를 추가했다. 한글 입력기를 직접 조작하거나 실제 UI를 시각적으로 확인하는 테스트는 수행하지 않았다.

native EDIT 위에 composition이 덮이지 않도록 target을 child 아래에 두는 근거: [Microsoft CreateTargetForHwnd 레이어 문서](https://learn.microsoft.com/en-us/windows/win32/api/dcomp/nf-dcomp-idcompositiondesktopdevice-createtargetforhwnd).

## 4단계 메뉴바 실제 Windows 확인 항목

- [ ] Dock 메뉴에서 Show menu bar/Hide menu bar 전환과 재시작 후 설정 복원을 확인한다.
- [ ] 실제 앱 이름/시각/배터리/기본 오디오 볼륨을 확인한다. 조회할 수 없는 수치는 표시하지 않는다.
- [ ] Search·Network·Sound·Date/Time·전원·About PC·작업 관리자·로그 메뉴가 실제 기능을 연다.
- [ ] popup을 열어도 상단 예약 높이가 바뀌지 않으며, 밖 클릭/Escape로 닫히고 기존 앱 포커스가 복원된다.
- [ ] 다른 AppBar/상단 작업표시줄이 있을 때 Shell이 제안한 좌표를 존중하고 최대화 창과 겹치지 않는다.
- [ ] 전체화면 진입 후 메뉴바가 앞으로 올라오지 않으며 종료 후 복귀한다.
- [ ] 보조 화면 선택, 배율/해상도 변경, 화면 분리, Explorer 재시작 시 예약과 표시 위치가 맞는다.
- [ ] Hide/정상 종료/초기화 실패 시 메뉴바 예약이 해제된다. 강제 종료/충돌 복구도 별도로 확인한다.

native smoke에 AppBar 예약 전후 작업영역 비교와 실제 surface 텍스트 그리기를 추가했다. 실제 Windows에서 실행하지 않았다. 기본 메뉴바 설정은 false다.

## 5단계 Stage Manager 실제 Windows 확인 항목

- [ ] Enable/Disable Stage Manager 설정을 바꾸고 재시작 후 반영을 확인한다. 활성화만으로 창을 최소화하지 않는다.
- [ ] 작은 가장자리 핸들 외 바탕화면/앱 입력을 막지 않는다. 펼침/접힘 표시 영역과 입력 영역이 맞는다.
- [ ] 열린 상태에서 헤더 pin/unpin 표시가 즉시 바뀌고, 외부로 이동하면 자동으로 접힌다.
- [ ] 실제 DWM 썸네일이 불투명 composition 배경에 가려지지 않는다. 종횡비/고배율/작은 해상도를 확인한다.
- [ ] 최소화/보호 창은 마지막 프레임 또는 제한된 표시일 수 있다. 일시 오류 후 3초 대기/다시 펼침으로 재시도한다.
- [ ] 카드 클릭은 해당 그룹을 활성화한 뒤 이전 실제 활성 그룹만 최소화한다. 활성화 실패 시 다른 창을 바꾸지 않는다.
- [ ] 여러 창의 우클릭 선택과 최대화 상태 보존, 빠른 종료/생성/PID 변경을 확인한다.
- [ ] 같은 이름의 앱과 같은 exe를 쓰는 별도 AUMID/PWA 그룹을 섞지 않는다.
- [ ] Stage 최소화 → 작업표시줄에서 수동 복원 → 사용자가 다시 최소화 → Stage 끄기 시 수동 최소화를 취소하지 않는다.
- [ ] Stage가 직접 최소화했던 창은 끄기/정상 종료 시 복원을 시도하며, 원래 최소화돼 있던 창은 유지한다. 실패 로그도 확인한다.
- [ ] 바탕화면 표시 시 선반이 유지되고, 실제 전체화면 앱에서는 숨었다가 복귀한다.
- [ ] 선택 모니터/배율/작업영역/페이지 변경 후 썸네일과 카드 클릭 좌표가 맞고 stale DWM 핸들이 남지 않는다.

native smoke는 실제 DWM 등록/속성 갱신과 축소 HWND 크기를 검사한다. 실제 카드 이미지와 이벤트 전달을 시각적으로 검증한 것은 아니다. 이 호스트에서는 native smoke를 실행하지 않았다.


## 6단계 패키지·성능 검증

현재 버전 `1.1.0-alpha.1`. 로컬 모델 테스트/대상 컴파일에 이어 Windows CI를 실행했다. 아래 상태는 2026-11-12 KST 최초 성공 실행 기준이며 실제 Windows 10/11 사용성은 별도 확인한다.

| 항목 | 현재 증거 |
|---|---|
| release 실행 파일·Inno 설치기 생성 | Windows CI 통과 |
| PowerShell AST 구문 검사 | Windows CI 통과 |
| 동일 payload 해시·설치 버전 검사 | Windows CI 통과, 다운로드 후 SHA-256 재확인 |
| 실행 중 재설치/제거, 설정 보존 | Windows CI 통과 |
| 이전 PowerShell 등록/자동 시작 정리 | Windows CI fixture 통과 |
| 실제 이전 공개 버전에서 업그레이드 | 미실행, fixture와 구분 |
| 자동 시작/재부팅 | 미실행 |
| CPU/working set | hosted Windows 유휴 30초 표본 기록, 아래 참고 |
| 프레임 시간/GPU 메모리/시각적 외형/IME | 미측정·미검증 |
| 코드 서명 | 없음 |

깨끗한 일회용 Windows 사용자 계정에서 `scripts/package-windows.ps1 -RunNativeSmoke`와 `scripts/test-installer.ps1`을 실행한다. CI의 수동 Windows Preview Package도 동일 경로를 사용한다. 패키징 실패 시 리뷰용 패키지를 업로드하지 않으며 검증 로그는 실패 후에도 업로드한다. Manifest의 `passed`는 해당 실행 단계가 성공한 후에만 쓴다. `visual_ime_monitor_validation`은 자동으로 passed로 바꾸지 않는다.

- [ ] Setup으로 기본 설치, 자동 시작 기본 해제, 시작 메뉴·선택한 바탕화면 바로가기를 확인한다.
- [ ] 실행 중 같은 버전 재설치/제거 시 정상 종료와 AppBar/DWM/창 상태 복원을 확인한다. 자동 검사는 종료 코드/컴포넌트 보고서까지이며 화면 복원은 직접 확인한다.
- [ ] 기존 공개 버전을 설치·사용한 후 새 버전으로 업데이트해 설정/실행 상태를 확인한다. 이전 등록 fixture를 실제 업그레이드 결과로 기록하지 않는다.
- [ ] 자동 시작을 선택하고 Windows 재부팅 후 한 인스턴스만 실행되는지 확인한다. 선택 해제 후 재부팅에서도 실행되지 않아야 한다.
- [ ] 제거 후 실행 파일·등록·바로가기가 없고 사용자 설정·로그와 사용자가 추가한 파일은 남는지 확인한다.
- [ ] 다른 폴더에서 실행 중인 런처에 설치기가 종료 요청을 보내지 않는지 확인한다.

성능 표본은 실제 프로세스 PID를 지정해 수집한다. 실행 파일 경로와 일치하지 않으면 거부하며 다른 프로세스에 입력하거나 설정을 바꾸지 않는다.

```powershell
.\scripts\measure-process.ps1 -LauncherPid 1234 -Executable 'C:\Users\tester\AppData\Local\Programs\MacLauncher\win-launcher.exe' -Scenario idle -Seconds 30 -OutputPath target\validation-logs\idle.json
# 같은 명령에서 Scenario를 animation/search로 바꾸고 해당 동작을 실제 수행한다.
```

각 조건에서 화면 해상도/배율·모니터 수·핀 수·검색어·옵션 활성화 여부·정확한 동작을 함께 기록한다. warm-up 후 유휴 측정 중에는 마우스를 Dock 밖에 둔다. 애니메이션은 반복 hover/launch, 검색은 반복 열기/입력/결과 전환 조건을 기록한다. 스크립트는 실제 상호작용을 감지하지 않으므로 결과의 `interaction_confirmed:false`를 보고 조건을 직접 확인한다.

CPU는 단일 코어 기준과 논리 프로세서 수로 나눈 전체 머신 기준을 모두 기록한다. 메모리는 250ms 간격 프로세스 working set 평균/최대 표본이며 전체 GPU 메모리나 private bytes로 해석하지 않는다. 이 수치로 FPS를 추정하지 않는다. 프레임 간격·긴 프레임·표시 지연은 별도 Windows 프레임 캡처의 원본 기록을 첨부하고 측정 도구/이벤트 의미를 기재한다. 아직 측정하지 않은 프레임 시간은 `not-measured`로 남긴다.


## 2026-11-12 KST 최초 Windows CI 성공

[실행 43045554382](https://github.com/yubin0210/mac-windows-launcher/actions/runs/43045554382), 소스 `5e9bd9d9e144c3c8ba8673d9ae7b6a8258c0cb7e`, 버전 `1.1.0-alpha.1`. 환경: GitHub hosted **Windows Server 2025** (`windows-2025-vs2026`, NT 10.0.26100), 논리 CPU 4개, Microsoft Hyper-V Video 드라이버 10.0.26100.1150, Rust 1.99.1, Inno Setup 6.7.1. Windows 10/11 실사용 PC 검증 결과로 해석하지 않는다.

- MSVC release workspace **204개**, 명시적 native smoke **8개** 통과.
- PowerShell AST, 동일 release exe 버전/해시, Inno 컴파일, 실제 새 설치 통과.
- 메뉴바·Stage 활성화 상태의 실제 런처 기동/정상 종료와 실행 중 재설치·제거 통과.
- 설정 보존, 자동 시작 선택/해제, 구형 등록 fixture 정리, 다른 앱 바로가기 및 설치 폴더 내 사용자 파일 보존 통과.
- 다운로드한 설치기/exe 해시를 manifest와 다시 대조했다. `source_dirty:false`, `unit_tests/native_api_smoke/installer_lifecycle/runtime_startup_smoke/legacy_migration_fixture:passed`.

이 과정에서 실제 실패 두 가지를 수정하고 재실행했다. 창 생성의 초기 Paint를 합성 큐 테스트가 입력 이벤트로 세던 문제는 주입 전에 초기 메시지를 비워 해결했다. RUNNER~1 짧은 경로와 바로가기의 긴 경로가 달라 자동 시작 opt-out에 실패하던 문제는 GetLongPathNameW로 기존 경로를 정규화해 해결했다. [Windows API 근거](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getlongpathnamew).

유휴 fixture 표본은 빈 고정 목록, 메뉴바/Stage 켜짐, `--smoke-test --wait-for-quit` 상태다. 30.16초/119개 표본에서 단일 코어 기준 CPU **0.207%**, 전체 머신 기준 **0.052%**, working set 평균 **46.1 MiB**, 최대 표본 **46.3 MiB**였다. 실제 hover/검색 입력은 하지 않았다(`interaction_confirmed:false`). 단일 가상 환경 표본이며 성능 보장·FPS·GPU 메모리 수치로 사용하지 않는다. 초기 성공 빌드의 exe SHA-256은 `b12d6f0b54dfdf40aa2c303415fd1a3d65480df38aee69d13e38a8e23815aa00`이며 이후 빌드는 각 manifest 해시로 식별한다.

화면 외형·한국어 IME·실제 Explorer 재시작·다중 모니터·절전 복귀·장치 손실·재부팅·이전 공개 바이너리에서 업그레이드·애니메이션/검색 성능 및 프레임 시간은 여전히 확인하지 않았다. 이 CI 성공으로 해당 체크박스를 완료 처리하지 않는다. 서명 없는 프리뷰 artifact이며 정식 Release는 발행하지 않았다.
