# 要件定義

## 目的

Vineは、Windows向けVN／ADVの元のPE executable、DLL、assetを変更せず、ブラウザ内のWindows userspace互換runtimeで再ホストする。active targetはfingerprint済みのサクラノ詩`BGI.exe`であり、Chrome上で全編を不自由なくプレイできることを完成条件とする。

現在の到達点と再現commandは[compact current state](current-checkpoint.json)を正本とする。API call数、instruction数、report hash、過去のcheckpoint値は製品要件ではない。

## Product acceptance

次を順に実タイトルで成立させる。

1. readable non-black frame、またはpointer／keyboard actionへの明確なvisible response
2. BGM／効果音を含むprimary audible output
3. sourceと分離したsave／設定／registry write、browser再起動後のload
4. 起動から選択肢、画面遷移、音声、save／loadを含むrepresentative flow
5. 全シナリオ、ending、long-runを未対応機能停止や破損なしで完了

## Functional requirements

### Import and launch

- 利用者が選んだgame directoryを検査し、PE32／i386 targetと必要fileを特定する。
- executable、DLL、assetのimmutable snapshotとfingerprintから起動する。
- malformed PE、未対応machine、容量超過、欠落dependencyを理由付きで拒否する。
- sourceをoverlayでshadow、self-update、persistent mutationしない。

### Execution and compatibility

- 必要なx86、x87、MMX、SSE／SSE2 instructionとx86 ABI、guest callbackを実行する。
- PE section、relocation、import／export、DLL lifecycle、TLS、process／thread bootstrapをreached範囲で提供する。
- Kernel32、User32、GDI32、GDI+、COM、bounded D3D9、DirectInput 8、DirectSound、到達WinMM／locale APIのcaller-visible semanticsを提供する。
- result、LastError、output、identity、lifetime、orderingをcallerが観測する範囲で正しく実装する。
- 未対応variantは具体的なunsupported faultにし、成功を偽らない。

### Memory and state

- `GuestMemory`がguest bytes、mapping、owner、protection、generationを所有する。
- guest pointer、range、size、handle、COM pointerをhost使用前に検査する。
- heap、stack、virtual memory、thread、object、handle、COM lifetimeをRust側で管理する。
- 複数stateやpersistent bytesをまたぐmutationはpreflight後にcommitし、失敗をatomicにする。

### Window, graphics, input, audio

- logical window、message queue、timer、focus、cursor、keyboard／pointer、DirectInputをRust authorityで接続する。
- GDI／GDI+／D3D9 frontendとbrowser rendererを分離し、Canvas／WebGLで実画面をpublishする。
- DirectSound／WinMM frontendとWeb Audio transportを分離し、再生、停止、loop、volume、clockのreached semanticsを提供する。
- browser側はWindows stateの第二authorityを持たない。

### Storage

- source mountはread-only、save／設定／registryはtitle／profile scoped writable overlayとする。
- commit、close、restart、load、quota、cancel、stale generationをfailure-atomicに扱う。
- browser site data消去やpermission喪失を診断できるようにする。

### Diagnostics

- forward runは最初のunsupported／fault、caller context、LastError、current thread、bounded semantic tail、latest Present、input／audio／storage summary、pending workを小さなreportとして残す。
- report、save、private binary、raw traceは`.work/`へ置きGitへ追加しない。

## Non-functional requirements

- 通常のrelease forward runは原則10分以内とする。
- UI threadを長時間blockせず、Worker cancellation／watchdog後にchild processやleaseを残さない。
- source file hashは実行前後で不変である。
- save corruption、partial commit、wrong-title／wrong-profile accessを防ぐ。
- performance最適化はversioned workloadの測定結果に基づく。

## Scope

Active scopeはPE32／i386、required Win32／COM、GDI／GDI+、bounded D3D9、DirectInput 8、DirectSound／reached WinMM、Windows-932、Chrome、local immutable sourceとwritable overlayである。

PE32+／x64、Win64、D3D11／DXGI、XAudio2、Unity、Windows kernel／driver／service、OS全体の仮想化、multi-title一般化、public releaseはdeferredである。
