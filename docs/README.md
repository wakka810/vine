# Vine documentation

<!-- BEGIN CURRENT CHECKPOINT REFERENCE -->
> 変動するcurrent status（API contract数、call数、memory generation、next stop、
> report identity、canonical command）は
> [`current-checkpoint.json`](current-checkpoint.json)だけを正本とする。
> 最新値はルートREADMEのgenerated Current Statusを参照する。
<!-- END CURRENT CHECKPOINT REFERENCE -->

## 文書ステータス

- ステータス: Proposed
- 開発形態: 1 人、完全個人用
- 主目標: fingerprint 済みサクラノ詩 `BGI.exe` の full-play runtime

このディレクトリは、Vine を「Windows 向け VN/ADV 実行ファイルを、元の配布ファイルを原則として変更せずブラウザ上で再ホストする Windows userspace 互換ランタイム」として開発するための基準文書である。

現在状態の正本は[`current-checkpoint.json`](current-checkpoint.json)であり、ルート
READMEのCurrent Statusはそこから生成する。本書と個別checkpoint文書はarchitecture、観測理由、
non-claimsを記録するが、current call数やnext stopの第二の正本にはしない。

## プロジェクトの位置づけ

```text
Browser-native Windows userspace compatibility runtime
for Windows VN/ADV executables
```

日本語では、Windows 向け VN/ADV 実行ファイルをブラウザ上で再ホストするための、Web ネイティブな Windows userspace 互換ランタイムと位置づける。

```text
Windows PE32 / PE32+ executable and DLL
                  |
                  v
            PE image loader
                  |
                  v
         x86 / x86_64 execution
                  |
                  v
       Win32 / Win64 compatibility
                  |
                  v
  GDI / DirectX / audio compatibility
                  |
                  v
       Browser Web API backends
```

初期実装は PE32 / i386 と Win32 を基盤とし、GDI、DirectSound、サクラノ詩が要求する bounded D3D9／DirectInput 8 subset を active scope とする。DirectDraw は同系統 title との比較 branch として扱う。PE32+ / x86_64、D3D11 / DXGI、XAudio2、Unity 系 runtime は将来の独立した成立性検証対象である。

active completion target は第三者向けの汎用 runtime や multi-title release ではなく、fingerprint 済みのサクラノ詩を手元の Chrome で全編不自由なくプレイできる 1-title runtime である。短い scenario は段階的 checkpoint として使うが、最終完了の代用にはしない。拡張可能な Windows userspace 互換境界は維持するが、個人開発の critical path に一般化や公開 release の作業を置かない。

## 文書一覧

| 文書 | 役割 | 主な内容 |
|---|---|---|
| [current-state-manifest.md](current-state-manifest.md) | current stateをどこで更新するか | schema区分、README生成、canonical再現command、検証境界 |
| [private-target-gate.md](private-target-gate.md) | private primary targetをcommitへどう結ぶか | clean implementation tree、Node／Chrome semantic gate、small evidence manifest、performance分離 |
| [corpus-inventory.md](corpus-inventory.md) | 何を最初に動かすか | ローカル title inventory、primary target、静的依存、調査待ち evidence |
| [toolchain-spike.md](toolchain-spike.md) | 何を再現生成できるか | LLVM／Wasm baseline、PE32 fixture、次の engine 比較条件 |
| [x86-engine-scorecard.md](x86-engine-scorecard.md) | どの x86 engine を先に試すか | 4 候補の比較、v86 実行 evidence、adapter blocker と採用 gate |
| [adr-003-guest-memory.md](adr-003-guest-memory.md) | guest memoryをどこが所有するか | Rust byte authority、v86 execution mirror、slice同期／failure policy |
| [process-address-space-allocator.md](process-address-space-allocator.md) | process rangeをどこで予約するか | allocation identity、v1 control／heap registry、search reservation、release policy |
| [com-iunknown-foundation.md](com-iunknown-foundation.md) | x86 COM identity／lifetimeをどこが所有するか | IUnknown ABI、vtable／UD2 thunk、HRESULT、shared refcount、Browser resource分離 |
| [d3d9-frontend.md](d3d9-frontend.md) | bounded D3D9と将来Direct3D frontendをどこで分けるか | root identity、COM trap routing、version別frontend、CreateDevice時の`RuntimeGraphics` boundary |
| [gdi-frontend.md](gdi-frontend.md) | GDI identity／surfaceと将来graphics共有層をどこで分けるか | HDC／stock object／logical surface authority、failure ordering、Direct3D／renderer分離 |
| [win32-compatibility-policy.md](win32-compatibility-policy.md) | Win32 semanticsと到達profileをどう分けるか | single handler ID、versioned bounded policy、evidence-only BGI identity |
| [winmm-compatibility-policy.md](winmm-compatibility-policy.md) | WinMM capabilityとtimer stateをどう分けるか | policy-owned TIMECAPS、timer-resolution owner、共有process tick authority |
| [runtime-input-state.md](runtime-input-state.md) | guest-visible input stateをどこが所有するか | logical mouse-button mapping、configure-once、stale plan拒否、browser authority分離 |
| [guest-memory-ingress.md](guest-memory-ingress.md) | production memory ingressが何を所有するか | sparse region／page API、fault／invalidation、fixture／BGI layout evidence |
| [memory-delta-and-transactions.md](memory-delta-and-transactions.md) | mutationと失敗をどう同期するか | bidirectional dirty pages、binary transport、generation ack、rollback |
| [memory-writeback.md](memory-writeback.md) | guest writeをどうRust authorityへ戻すか | PTE Dirty discovery、atomic multi-page reconciliation、two-slice evidence |
| [generic-api-dispatcher.md](generic-api-dispatcher.md) | API固有checkpointを共通runtimeへどう移すか | RuntimeState、manifest-driven generic runner、dynamic endpoint dispatch |
| [bgi-generic-runtime-reentry.md](bgi-generic-runtime-reentry.md) | frozen BGI evidenceをproduction runtimeへどう戻すか | process module graph、automatic binary writeback、21 generic calls、critical-section first stop |
| [bgi-critical-section-synchronization.md](bgi-critical-section-synchronization.md) | reached x86 critical sectionをどう所有するか | native 24-byte oracle、process-owned recursion、14 BGI initializations、DecodePointer first stop |
| [bgi-primary-main-loop-progression.md](bgi-primary-main-loop-progression.md) | main-loopのscheduler round tripをどう検証するか | Rust-owned thread selection、two API-boundary context switches、Node／Chrome／benchmark evidence、LeaveCriticalSection stop |
| [bgi-leave-critical-section.md](bgi-leave-critical-section.md) | main-threadのcritical-section releaseをどう検証するか | Rust-owned release plan、24-byte unlocked state、typed Worker completion、Node／Chrome／benchmark evidence、EnterCriticalSection stop |
| [bgi-pointer-decoding.md](bgi-pointer-decoding.md) | process pointer tokenをどう逆引きするか | native inverse evidence、strict owned-token lookup、FlsAlloc dynamic stop |
| [bgi-fiber-local-storage-allocation.md](bgi-fiber-local-storage-allocation.md) | reached FLS slotをどう所有するか | native callback/index evidence、bounded process slots、dynamic dispatch、HeapAlloc first stop |
| [bgi-private-heap-allocation.md](bgi-private-heap-allocation.md) | reached private-heap blockをどう所有するか | native flags/size evidence、transactional guest backing、FlsSetValue first stop |
| [bgi-fiber-local-storage-value.md](bgi-fiber-local-storage-value.md) | reached FLS valueをどこへ保存するか | native BOOL/LastError evidence、selected-thread state、InterlockedIncrement first stop |
| [bgi-interlocked-increment.md](bgi-interlocked-increment.md) | reached atomic LONGをどう更新するか | native wrapping evidence、checked guest write、second HeapAlloc variant stop |
| [bgi-private-heap-second-allocation.md](bgi-private-heap-second-allocation.md) | reached second heap blockをどう保持するか | versioned size gate、unsealed-owner delta、GetStdHandle first stop |
| [bgi-standard-input-handle.md](bgi-standard-input-handle.md) | reached standard input identityをどう所有するか | native result／consumer、logical handle、GetFileType first stop |
| [bgi-file-type.md](bgi-file-type.md) | reached stdin typeをどう分類するか | native FILE_TYPE_CHAR、logical lookup、stdout first stop |
| [bgi-standard-output-handle.md](bgi-standard-output-handle.md) | reached stdout identityをどう所有するか | versioned selector profile、logical stdout、output type first stop |
| [bgi-standard-output-file-type.md](bgi-standard-output-file-type.md) | reached stdout typeをどう分類するか | native FILE_TYPE_PIPE、logical lookup、MessageChannel yield、stderr first stop |
| [bgi-standard-error-handle.md](bgi-standard-error-handle.md) | reached stderr identityをどう所有するか | native result／consumer、logical stderr、error type first stop |
| [bgi-standard-error-file-type.md](bgi-standard-error-file-type.md) | reached stderr typeをどう分類するか | native FILE_TYPE_PIPE、logical lookup、SetHandleCount first stop |
| [bgi-set-handle-count.md](bgi-set-handle-count.md) | reached handle countをどう返すか | native result／discarded consumer、versioned count policy、GetCommandLineA first stop |
| [bgi-command-line-a.md](bgi-command-line-a.md) | reached ANSI command lineをどう公開するか | native pointer relation、process-owned ASCII view、GetEnvironmentStringsW first stop |
| [bgi-environment-strings-w.md](bgi-environment-strings-w.md) | reached wide environment blockをどう公開・解放するか | copy lifetime、double NUL、paired free、WideCharToMultiByte first stop |
| [bgi-wide-char-to-multi-byte.md](bgi-wide-char-to-multi-byte.md) | reached UTF-16 conversionをどう固定するか | host-locale-independent Windows-932、bounded dynamic output、HeapAlloc size-1 stop |
| [bgi-private-heap-size-1.md](bgi-private-heap-size-1.md) | reached one-byte private-heap allocationをどう固定するか | native pointer／consumer／lifetime relation、bounded flags=0 policy、GetLastError stop |
| [bgi-get-last-error.md](bgi-get-last-error.md) | reached thread LastErrorをどう返すか | zero-argument ABI、selected-thread authority、native sentinel oracle、TlsGetValue stop |
| [bgi-tls-get-value.md](bgi-tls-get-value.md) | reached TLS valueをどう返すか | set／unset／invalid semantics、selected-thread isolation、native PE32 fixture、FlsGetValue stop |
| [bgi-fls-get-value.md](bgi-fls-get-value.md) | reached FLS valueをどう返すか | thread-storage lifecycle、set／unset／invalid semantics、native PE32 fixture、SetLastError stop |
| [bgi-set-last-error.md](bgi-set-last-error.md) | reached LastError setterをどう適用するか | selected-thread write、native sentinel oracle、VOID/EAX classification、GetACP stop |
| [bgi-get-acp.md](bgi-get-acp.md) | reached active ANSI code pageをどう返すか | policy-owned Windows-932、LastError preserve、caller consumer、HeapAlloc size-544 stop |
| [bgi-private-heap-size-544.md](bgi-private-heap-size-544.md) | reached 544-byte private-heap allocationをどう固定するか | focused allocation observer、bounded flags=0 policy、full-buffer consumer、GetCPInfo stop |
| [bgi-get-cp-info.md](bgi-get-cp-info.md) | reached active code-page informationをどう書くか | guarded raw oracle、18-byte checked output、lead-byte consumer、MultiByteToWideChar stop |
| [bgi-multi-byte-to-wide-char.md](bgi-multi-byte-to-wide-char.md) | reached Windows-932 multibyte inputをどう変換するか | six-argument ABI、query／conversion、private-use mappings、GetStringTypeW stop |
| [bgi-get-string-type-w.md](bgi-get-string-type-w.md) | reached UTF-16 inputをどう分類するか | four-argument ABI、checked CTYPE1 WORD output、negative-count exclusion、LCMapStringW stop |
| [bgi-lc-map-string-w.md](bgi-lc-map-string-w.md) | reached UTF-16 tableをどうcase-mapするか | six-argument ABI、ja-JP lower／upper query-conversion、InterlockedDecrement stop |
| [bgi-interlocked-decrement.md](bgi-interlocked-decrement.md) | reached LONGをどうatomic decrementするか | one-argument ABI、checked wrapping update、GetModuleFileNameA stop |
| [bgi-get-module-file-name-a.md](bgi-get-module-file-name-a.md) | current executable pathをどこが所有しどうANSI copyするか | process bootstrap authority、bounded output、HeapAlloc size-16 stop |
| [bgi-private-heap-size-16.md](bgi-private-heap-size-16.md) | virtual path由来の16-byte bufferをどうheapへ持つか | native size非import、dirty-only allocation、HeapAlloc size-4 stop |
| [bgi-private-heap-size-4.md](bgi-private-heap-size-4.md) | empty environmentのterminator tableをどうheapへ持つか | native environment非import、zeroed allocation、HeapFree stop |
| [bgi-private-heap-free-size-1.md](bgi-private-heap-free-size-1.md) | reached one-byte blockをどう安全にfreeするか | exact heap／pointer lifetime、transactional poison、HeapAlloc size-128 stop |
| [bgi-private-heap-size-128.md](bgi-private-heap-size-128.md) | fixed 32-entry tableをどうheapへ持つか | native reuse非import、128-byte zeroed allocation、IsProcessorFeaturePresent stop |
| [bgi-is-processor-feature-present.md](bgi-is-processor-feature-present.md) | CPU feature queryをhost discoveryなしでどう返すか | explicit presence map、BOOL／LastError、SetUnhandledExceptionFilter stop |
| [bgi-set-unhandled-exception-filter.md](bgi-set-unhandled-exception-filter.md) | guest top-level filterをどこで所有するか | process-owned replacement、executable preflight、HeapSize stop |
| [bgi-heap-size.md](bgi-heap-size.md) | private-heap sizeをどのauthorityから返すか | exact live allocation、requested size、InitializeCriticalSection stop |
| [bgi-initialize-critical-section.md](bgi-initialize-critical-section.md) | one-argument critical sectionをどう初期化するか | native 24-byte state、VOID ABI、versioned raw-field policy、initial stack-growth fault |
| [bgi-load-icon-a.md](bgi-load-icon-a.md) | module iconをどう選択・所有するか | bounded PE resources、shared logical HICON、native／Node／Chrome evidence |
| [bgi-load-cursor-a.md](bgi-load-cursor-a.md) | system cursor identityをどこで所有するか | shared logical HCURSOR、LastError preserve、process teardown、native／Node／Chrome evidence |
| [bgi-register-class-ex-a.md](bgi-register-class-ex-a.md) | process-local window classをどこで所有するか | module/name＋ATOM identity、WndProc／resource references、failure atomicity、native／Node／Chrome evidence |
| [browser-runtime-worker.md](browser-runtime-worker.md) | Chrome内でruntime全経路をどこまで通せるか | Worker guest CPU、async completion、nested callback、pixel／audio／storage、cancel／watchdog、30分stress |
| [bgi-browser-worker.md](bgi-browser-worker.md) | current BGI sessionをChromeへ移せたか | Rust/Wasm authority、Node semantic oracle、memory digest、heartbeat、cancel／watchdog |
| [bgi-browser-benchmark.md](bgi-browser-benchmark.md) | BGI Browser workloadの性能／memoryはgate内か | 3分反復、session／slice分布、main／Worker heap slope、linear memory、Wasm crossing |
| [pe-loader-ingress.md](pe-loader-ingress.md) | immutable PE32 planをどこまで作れるか | Rust workspace、bounded parser／plan、fixture contract、BGI W7 stop |
| [pe-materialization.md](pe-materialization.md) | PE image bytesをどこまで構築できるか | source copy、zero-fill、`HIGHLOW`、IAT-original verification、BGI stop |
| [pe-import-binding.md](pe-import-binding.md) | runtime import endpointをどこまで所有できるか | all-import planning、atomic IAT commit／rollback、unresolved retry、thunk arena／ID |
| [pe-unsupported-trap-binding.md](pe-unsupported-trap-binding.md) | 未実装static importをどうseal可能にするか | identity付きunsupported traps、controlled test、BGI full bind |
| [v86-entry-ingress.md](v86-entry-ingress.md) | production memoryをCPUへどこまで渡せるか | semantic reflection、invalidation ack、bounded stack、entry-to-thunk evidence |
| [v86-cdecl-ingress.md](v86-cdecl-ingress.md) | 最初のproduction serviceをどこまでreturnできるか | exact thunk metadata、fixed cdecl frame／return、guest cleanup、callback-service ingress |
| [v86-callback-return-ingress.md](v86-callback-return-ingress.md) | production callbackをどこまで往復できるか | separate control owner／sentinel、frame ID、checked stack、exact callback return |
| [v86-outer-return-ingress.md](v86-outer-return-ingress.md) | outer callback serviceをどこまで完了できるか | frame retirement、fixed outer cdecl return、guest result／entry continuation、structured fetch fault |
| [v86-entry-return-ingress.md](v86-entry-return-ingress.md) | guest entry completionをどこまで所有できるか | typed process-entry sentinel、exact return classification、result／stack balance |
| [v86-stack-bootstrap-ingress.md](v86-stack-bootstrap-ingress.md) | initial stackをどこまで所有できるか | Rust stack owner／region、bootstrap metadata、reflection、adapter-local stack removal |
| [v86-bootstrap-fs-first-stop.md](v86-bootstrap-fs-first-stop.md) | i386 FS bootstrapの未実装境界はどこか | native TEB／PEB self-oracle、import-free reflection、zero-FS-base exact `#PF` |
| [v86-bootstrap-fs-ingress.md](v86-bootstrap-fs-ingress.md) | 最小TEB／PEBをどこまで所有できるか | typed Rust regions、exact fields、FS-base restore、entry completion |
| [v86-bootstrap-params-first-stop.md](v86-bootstrap-params-first-stop.md) | process parametersの未実装境界はどこか | native command-line oracle、PEB `+0x10` zero、explicit result `2` |
| [v86-bootstrap-params-ingress.md](v86-bootstrap-params-ingress.md) | 測定済みcommand-line pathをどこまで所有できるか | typed regions、exact UTF-16 bytes、source-v86 entry completion |
| [v86-bgi-first-trap.md](v86-bgi-first-trap.md) | primary titleをどこまで実行できるか | bounded 388-page transfer、VGA hole回避、BGI entry、first runtime-owned trap |
| [v86-bgi-first-win32-call.md](v86-bgi-first-win32-call.md) | first reachable APIをどこまでreturnできるか | controlled clock、stack page reconciliation、`FILETIME` delta、stdcall return、next trap |
| [v86-bgi-second-win32-call.md](v86-bgi-second-win32-call.md) | second reachable APIをどこまでreturnできるか | controlled process identity、zero-argument stdcall、generation-preserving reconciliation、next trap |
| [v86-bgi-third-win32-call.md](v86-bgi-third-win32-call.md) | third reachable APIをどこまでreturnできるか | controlled thread identity、zero-argument stdcall、generation-preserving reconciliation、next trap |
| [v86-bgi-fourth-win32-call.md](v86-bgi-fourth-win32-call.md) | fourth reachable APIをどこまでreturnできるか | controlled monotonic sample、zero-argument stdcall、generation-preserving reconciliation、next trap |
| [v86-bgi-fifth-win32-call.md](v86-bgi-fifth-win32-call.md) | fifth reachable APIをどこまでreturnできるか | controlled 64-bit counter、generation 26 delta、BOOL return、next trap |
| [v86-bgi-sixth-win32-call.md](v86-bgi-sixth-win32-call.md) | sixth reachable APIをどこまでreturnできるか | native x86 observation、controlled `STARTUPINFOW`、generation 27 delta、VOID return、next trap |
| [v86-bgi-seventh-win32-call.md](v86-bgi-seventh-win32-call.md) | seventh reachable APIをどこまでreturnできるか | native heap-policy observation、exact four-argument variant、generation-preserving BOOL return、next trap |
| [v86-bgi-eighth-win32-call.md](v86-bgi-eighth-win32-call.md) | eighth reachable APIをどこまでreturnできるか | native heap lifecycle observation、typed logical heap handle、generation-preserving `HeapCreate` return、next trap |
| [v86-bgi-ninth-win32-call.md](v86-bgi-ninth-win32-call.md) | ninth reachable APIをどこまでreturnできるか | native module-handle observation、bounded UTF-16 lookup、typed logical loaded-module handle、next trap |
| [v86-bgi-tenth-win32-call.md](v86-bgi-tenth-win32-call.md) | tenth reachable APIをどこまでreturnできるか | native dynamic-export observation、register-indirect IAT provenance、callable `FlsAlloc` endpoint、next lookup |
| [v86-bgi-eleventh-win32-call.md](v86-bgi-eleventh-win32-call.md) | eleventh reachable APIをどこまでreturnできるか | ordered endpoint pair、persistent ESI provenance、first endpoint guest store、`FlsGetValue` return／next lookup |
| [v86-bgi-twelfth-win32-call.md](v86-bgi-twelfth-win32-call.md) | twelfth reachable APIをどこまでreturnできるか | ordered endpoint triple、prior endpoint guest stores、`FlsSetValue` return／next lookup |
| [v86-bgi-thirteenth-win32-call.md](v86-bgi-thirteenth-win32-call.md) | thirteenth reachable APIをどこまでreturnできるか | ordered four-endpoint page、`FlsFree` return／store、`TlsAlloc` next trap |
| [v86-bgi-fourteenth-win32-call.md](v86-bgi-fourteenth-win32-call.md) | fourteenth reachable APIをどこまでreturnできるか | typed logical TLS index、`TlsAlloc` return、`TlsSetValue` first consumer |
| [v86-bgi-fifteenth-win32-call.md](v86-bgi-fifteenth-win32-call.md) | fifteenth reachable APIをどこまでreturnできるか | initial-thread TLS endpoint value、three-page reconciliation、`EncodePointer` next trap |
| [v86-bgi-sixteenth-win32-call.md](v86-bgi-sixteenth-win32-call.md) | sixteenth reachable APIをどこまでreturnできるか | focused pointer evidence、controlled encoded-NULL token、next non-NULL call |
| [v86-bgi-seventeenth-win32-call.md](v86-bgi-seventeenth-win32-call.md) | seventeenth reachable APIをどこまでreturnできるか | executable initializer source、second token／guest store、`FlsAlloc` endpoint next call |
| [windows-development.md](windows-development.md) | Windows で何から始めるか | toolchain、Frida trace、最初の縦断 checkpoint、test 方針 |
| [windows-native-evidence.md](windows-native-evidence.md) | Windows で何が実測済みか | W0 version、BGI trace v1／v2、selected IDirect3D9 root、W1／W2／W3、W7 loader／memory／entry／stack／FS ingress |
| [requirements.md](requirements.md) | 何を、どこまで作るか | 目的、スコープ、機能要件、非機能要件、personal target 定義 |
| [target-architecture.md](target-architecture.md) | どの境界で作るか | コンポーネント、実行モデル、同期・非同期境界、runtime robustness |
| [adr-006-vfs-local-server.md](adr-006-vfs-local-server.md) | 最初のVFS／save backendをどこに置くか | localhost same-origin server、immutable source、SQLite generation commit、restart vertical slice |
| [risks-and-open-questions.md](risks-and-open-questions.md) | 何が未確定で、何が失敗要因か | 意思決定待ち、リスク台帳、停止条件、ADR 候補 |
| [testing-and-acceptance.md](testing-and-acceptance.md) | 何をもって動作したと判定するか | テスト階層、fixture、品質指標、リリース判定 |
| [checks-and-ci.md](checks-and-ci.md) | canonical gateをどう実行するか | fast／fixture E2E／benchmark／adapter／private corpus、Linux GitHub Actions、fuzz |
| [v86-browser-benchmark.md](v86-browser-benchmark.md) | v86を継続するか | Chrome Worker CPU／ABI／memory／audio／background／steady-memory gate |
| [runtime-object-model.md](runtime-object-model.md) | runtime object identityをどう所有するか | typed handle、generation、close／stale、waitable process／thread lifetime |
| [runtime-module-graph.md](runtime-module-graph.md) | process module identityと順序をどう所有するか | canonical identity、dependency／reference、attach／detach order |
| [runtime-module-images.md](runtime-module-images.md) | mapped guest module／export／resourceをどう所有するか | sealed image identity、name／ordinal／forwarder、load-base resource catalog、failure atomicity |
| [runtime-dll-lifecycle.md](runtime-dll-lifecycle.md) | guest importとDLL process callbackをどう結ぶか | direct／runtime IAT、static dependency、attach unwind／retry、reverse detach |
| [runtime-process-thread-lifecycle.md](runtime-process-thread-lifecycle.md) | process／thread terminationをどう所有するか | bootstrap／running、exit request／completion、atomic signal order |
| [runtime-thread-registry.md](runtime-thread-registry.md) | logical thread stateをどう分離するか | controlled second thread、selection／exit、logical affinity／COM apartment、deterministic all-lane termination |
| [runtime-wait-queue.md](runtime-wait-queue.md) | waitable objectとschedulerをどう接続するか | one-object registration、signal／wake event、already-signaled／exit cancellation |
| [runtime-message-queue.md](runtime-message-queue.md) | thread messageをどこが所有するか | bounded raw FIFO、empty block／first-message wake、ordered dequeue／exit cleanup |
| [runtime-synchronization.md](runtime-synchronization.md) | process synchronization stateをどこが所有するか | private critical-section／named-mutex collections、object-table coordinator、plan／commit atomicity |
| [bgi-multi-byte-to-wide-char-cp-acp.md](bgi-multi-byte-to-wide-char-cp-acp.md) | CP_ACP NUL終端変換をどこまで返せるか | native query／conversion、policy selector、Node／Chrome 279-call equivalence、next stop |
| [bgi-set-current-directory-a.md](bgi-set-current-directory-a.md) | process current directoryをどこまで返せるか | native path／oracle、private process identity、Node／Chrome 280-call equivalence、CreateMutexA stop |
| [bgi-create-mutex-a.md](bgi-create-mutex-a.md) | named mutex creationをどこまで返せるか | native new／existing oracle、private synchronization and typed-handle lifetime、Node／Chrome 284-call equivalence、GetCurrentThread stop |
| [bgi-get-current-thread.md](bgi-get-current-thread.md) | current-thread pseudo-handleをどう解決するか | native pseudo-handle／LastError oracle、selected-thread typed-object relation、captured Node／Chrome equivalence |
| [bgi-set-thread-affinity-mask.md](bgi-set-thread-affinity-mask.md) | logical affinityと進行するprocess clockをどう所有するか | native mask／calibration oracle、thread／process plan-commit、captured Node／Chrome／benchmark evidence |
| [bgi-co-initialize.md](bgi-co-initialize.md) | reached COM apartment初期化をどこへ置くか | native HRESULT／balance oracle、per-thread counted apartment、Node／Chrome equivalence |
| [bgi-get-system-default-lang-id.md](bgi-get-system-default-lang-id.md) | system-default LANGIDをどこが所有するか | native WORD／consumer oracle、policy-owned `0x0411`、typed U16 zero-extension、Node／Chrome equivalence |
| [bgi-get-drive-type-a.md](bgi-get-drive-type-a.md) | virtual drive typeをどこから導出するか | native A–Z／LastError oracle、process-directory-derived fixed drive、host topology非import、Node／Chrome equivalence |
| [bgi-swap-mouse-button.md](bgi-swap-mouse-button.md) | reached mouse mappingをどう変更・復元するか | Rust input authority、previous-state BOOL、two-call restore、next structured stop |
| [bgi-time-get-dev-caps.md](bgi-time-get-dev-caps.md) | reached WinMM timer capabilityをどう返すか | native size／guard oracle、policy-owned TIMECAPS、clock非進行、next resolution stop |
| [bgi-time-begin-period.md](bgi-time-begin-period.md) | WinMM timer-resolution requestをどこが所有するか | native pairing oracle、Rust timer plan／commit／teardown、Node／Chrome equivalence |
| [bgi-time-get-time.md](bgi-time-get-time.md) | multimedia tick sampleをどこが所有するか | shared process tick plan／commit、wrapping native oracle、Node／Chrome／benchmark evidence |
| [bgi-get-version-ex-a.md](bgi-get-version-ex-a.md) | size-versioned ANSI OS情報をどこが所有するか | guarded native layout oracle、policy-owned virtual version、atomic read-merge-write、Node／Chrome equivalence |
| [bgi-create-waitable-timer-a.md](bgi-create-waitable-timer-a.md) | waitable timer identity／lifetimeをどこが所有するか | native initial-state oracle、Rust timer payload＋typed object handle、atomic close／process teardown、Node／Chrome／benchmark evidence |
| [bgi-load-icon-a.md](bgi-load-icon-a.md) | mapped module iconをどう選び所有するか | PE resource catalog、shared logical HICON、native selection oracle、process teardown |
| [bgi-load-cursor-a.md](bgi-load-cursor-a.md) | predefined cursor identityをどこが所有するか | shared logical HCURSOR、LastError oracle、resource namespace分離、process teardown |
| [bgi-register-class-ex-a.md](bgi-register-class-ex-a.md) | process-local window classをどう登録するか | module/name identity、logical ATOM、WndProc／resource metadata、failure atomicity |
| [bgi-get-system-metrics.md](bgi-get-system-metrics.md) | initial window metricsをどこから返すか | Rust display authority、bounded non-client policy、caller geometry oracle、Node／Chrome equivalence |
| [runtime-heap-backing.md](runtime-heap-backing.md) | private heap bytes／lifetimeをどこが所有するか | bounded guest arena、allocation ID／alignment／initialization、free／reuse、heap／process teardown |
| [runtime-vfs.md](runtime-vfs.md) | immutable source file stateとasync byte serviceをどう分離するか | Rust manifest／path／typed handle／cursor authority、same-origin range adapter、failure atomicity、cancel／teardown |
| [roadmap.md](roadmap.md) | どの順序で実装するか | 依存関係、フェーズ、タスク分解、各フェーズの受け入れ条件 |

推奨する読み順は、corpus 台帳、要件、目標アーキテクチャ、リスク、検証、ロードマップの順である。ロードマップ内の `REQ-*`、`NFR-*`、`R-*`、`Q-*`、`AC-*` は各文書の識別子を参照する。

## 開発上の原則

1. タイトル固有のエンジン移植ではなく、PE と Windows userspace API の互換境界を実装する。
2. Windows API／DirectX API の必要部分を browser Web API へ直接接続する。
3. ブラウザでネイティブ x86 コードを直接実行せず、隔離された guest 実行器で命令を emulate または translate する。
4. 最初から広い Win32 互換を名乗らず、機械可読な API 実装状況とタイトル別互換性を記録する。
5. 実ゲームより先に、合成 PE fixture で各層を検証する。
6. malformed input、無限 loop、resource 上限を合成 fixture で再現可能に検査する。
7. 将来の PE32+ / x86_64 / D3D11 対応点は確保するが、サクラノ詩 prototype の設計を抽象化しすぎない。
8. Windows native trace で到達した経路から、広く浅い vertical checkpoint を一つずつ通す。subsystem の全面実装を先行させない。
9. automated test は各実装境界につき代表 contract 1 本を原則とし、実 regression、曖昧な fault／ABI semantics、title failure を最小化した場合だけ増やす。

## 用語

| 用語 | 意味 |
|---|---|
| guest | Vine 内でロード、実行される Windows 実行ファイルと、その仮想アドレス空間 |
| host | ブラウザ、Web API、および Vine の JavaScript/Wasm 実行環境 |
| compatibility runtime | PE loader、CPU 実行系、ABI、Windows API facade、Web backend の総体 |
| synthetic DLL | Microsoft の DLL バイナリを含まず、Vine が export と API 挙動を提供する論理 DLL |
| fixture | 挙動検証のために作成した小さな PE 実行ファイル |
| title corpus | 互換性検証に使用する代表タイトル群 |
| compatibility profile | タイトル、必要 API、実装状態、既知の制約、検証結果を結び付ける記録 |
| vertical slice | PE load から CPU、API bridge、Web backend までを最小機能で貫通させる検証 |

## ステータス表記

| 表記 | 意味 |
|---|---|
| Proposed | 合意前の提案 |
| Accepted | ADR またはレビューで採用済み |
| In progress | 実装または検証中 |
| Implemented | 受け入れ条件を満たし、CI 証跡がある |
| Deferred | 現在のリリース範囲外 |
| Rejected | 理由を記録して不採用 |

文書内に明記がない項目は `Proposed` とみなす。機能を `Implemented` に変更するには、[testing-and-acceptance.md](testing-and-acceptance.md) に定義した証跡が必要である。

## 文書の更新規則

- 仕様や技術選択を確定するときは ADR を追加し、関連する `Q-*` を `Resolved` にする。
- マイルストーンの完了は、タスクの実装完了ではなく、対応する `AC-*` の証跡で判定する。
- 対応 API を追加したら、API inventory、テスト、互換性 profile を同じ変更で更新する。
- 日程見積もりは成立性検証後に更新し、未検証の見積もりを納期として扱わない。
