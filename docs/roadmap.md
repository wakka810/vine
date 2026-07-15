# Vine ロードマップ

## 1. 目的と正本

この文書は、active completion targetまでに残る構造的な道筋だけを示す。API数、instruction数、
hash、benchmark値、exact next stop、完了済みcheckpoint履歴は複製しない。

- 現在状態、active profile、exact structured stop、artifact identity、canonical reproduction:
  [current-checkpoint.json](current-checkpoint.json)
- 製品要件と完了条件: [requirements.md](requirements.md)
- 安定したcomponent境界: [target-architecture.md](target-architecture.md)
- testの役割と受け入れ証跡: [testing-and-acceptance.md](testing-and-acceptance.md)
- 未解決の判断と構造的risk: [risks-and-open-questions.md](risks-and-open-questions.md)
- 過去のcheckpoint詳細: 各subsystem evidence文書とGit履歴

## 2. Active targetとscope

Active targetは、fingerprint済みのサクラノ詩をimmutable source snapshotからChromeで起動し、
boot、visible output、input、audible output、全scenario、save、browser restart、loadまで不自由なく
実行することである。

Active compatibility bandはPE32/i386、到達したWin32／COM、GDI／GDI+、bounded D3D9、
DirectInput 8、DirectSound／到達WinMM、Windows-932、file／registry persistence、Chrome backendとする。
PE32+／x64、D3D11／DXGI、XAudio2、Unity、multi-title、第三者向け配布は、明示的に再開するまで
deferredとする。

## 3. 計画規則

進捗は実装API数ではなくscenario milestoneで測る。Manifestのexact next structured stopは直近の
再現地点であり、孤立したendpoint patchを積み上げる許可ではない。

各stopは実装前に分類する。

- **Semantic leaf**: 既存subsystem内で閉じるbounded contract。caller behavior、negative case、
  regression evidenceを含めて閉じる。
- **Subsystem gateway**: thread／wait、VFS、window／message、graphics、audio、storage等、新しいownerや
  lifecycle境界を要求する最初の到達点。個別API追加前に最小vertical sliceとexit gateを定義する。
- **Feasibility blocker**: browser capability、corpus requirement、correctness、performance／memory条件が
  設計前提を脅かすもの。downstream実装より先にevidenceとscope判断を固定する。

Primary implementation checkpointは常に一件とする。後回しにすると大量の実装が推測になる
high-impact unknownだけは、到達前でもbounded spikeを許可する。

## 4. 確立済みの基盤

次は残タスクではなく、以後の計画が前提とするarchitectureである。

- Rustがguest-visible memory、process／thread、scheduler lane、object／handle、module、heap、
  synchronization、compatibility semanticsを所有する。
- Runtime Workerはversioned bridgeを介してexecution orchestrationとbrowser adapterを所有し、
  Windows stateを複製しない。
- `GuestMemory`がbyte authorityであり、execution engineはreflection／delta／writebackで同期する
  boundedかつ交換可能なmirrorである。
- Execution mirrorはRust提供limit内で成長し、released pageを回収し、枯渇をmutation前に拒否する。
- Heap compatibility policyはflags／rangeベースであり、historical exact-size profileは再現専用である。
- Cooperative Worker schedulerはthreadごとのCPU contextを保存／復元し、owner threadがhost requestで
  待機中に別guest threadを実行できる。General Win32 thread creation／wait coverageは後続gatewayである。
- Runtimeのcollection／allocatorは各private subsystemが所有し、`RuntimeKernel`はcross-subsystem
  preflightとtargeted commitだけを調整する。

## 5. 残る実装順序

| 順序 | Scenario milestone | 未解決の構造作業 | Exit gate |
|---|---|---|---|
| 1 | Primary bootを継続 | Manifest-selected semantic leafを次のsubsystem gatewayまで閉じる。caller-visible semanticsと既存authorityを維持する | Node／Chromeが同じ次stopへ到達し、cancel後にworkを残さずsource bytesを変更しない |
| 2 | Process／loader／execution completeness | 到達したdynamic DLL load／unload、thread creation／bootstrap、per-thread stack／TEB lifecycle、wait-any／wait-all、timeout、message、synchronization blocking、corpus-required exception behavior | Controlled multi-thread fixtureとprimary pathがcreate、block、wake、callback、exitを複数CPU contextで実行し、starvation／orphan workを残さない |
| 3 | Immutable sourceとpersistence | [ADR-006](adr-006-vfs-local-server.md)のsame-origin localhost serverを最初のbackendとし、決定的Windows path、source snapshot、file／registry handle、SQLite write-set、flush／close commitを接続する。最初に実asset readと一つのsave／restart／loadだけをverticalに通す | Assetはimmutable sourceから読み、browser／Worker／server再起動後にsaveを復元できる。Kill時はlast committed generationへ戻り、EXE／DLL／asset hashが不変 |
| 4 | Logical windowとmessage flow | HWND／class ownership、message dispatch、paint invalidation、focus、normalized keyboard／mouse、GDI object／DC lifetime、DIB／blit／text／font、到達GDI+ image path | Browser E2Eがfirst readable presentへ到達し、semantic／visual oracle付きでinputを消費する |
| 5 | Primary graphicsとinput | Corpus-bounded D3D9 COM／device／resource／state／present、DirectInput device／acquire／state、backend translation、device／context loss recovery | Primary first-presentとrepresentative scene／input checkpointがsilent unsupportedやunbounded growthなしで通る |
| 6 | Primary audio | DirectSound identity／buffer semantics、guest clockとaudio clock、cursor／notification wake、browser transport、activation／suspend／resume、到達WinMM／media command | BGM／voice／effectがoffline semantics、browser lifecycle E2E、long-run underrun／drift gateを通る |
| 7 | Save／loadとfull play | 同じfixture-first ruleで全到達gapを統合し、個人利用に必要なshell lifecycle、diagnostics、save management、recoveryを追加する | Clean profileでrepresentative flow、全route／ending、save、browser restart、loadを完了する |

Graphics、audio、storage、threadingがdownstreamの成立性を脅かす場合、そのrowより前にbounded spikeを
行ってよい。ただしparallel implementation trackを作らず、明示したstop conditionで終了する。

## 6. Cross-cutting gate

- **Threading／async host work**: request、thread、callback depth、generation、timeout、cancellation identityを
  検証する。Measured title workloadに必要なfairnessを持ち、waiting threadがCPU contextを独占しない。
- **Memory／snapshot**: full-play peak memoryとsnapshot sizeをversioned limit内に収める。Limit failureは
  guest memory、mapping、scheduler、allocator identityを部分更新しない。
- **Source／save safety**: source module／assetはlocalhost serverのread-only snapshot、save／registryは別の
  title-local SQLite overlayとする。Rust VFSとserver durabilityのauthorityを分け、flush／close commitを明示する。
- **Graphics／audio lifecycle**: browser focus、background、device／context loss、audio suspensionを
  host timingの推測ではなくrecoverable stateとして扱う。
- **Diagnostics**: unsupported gatewayはprivate raw traceをGitへ入れず、owner module／thread／resourceと
  first divergenceを報告する。
- **Performance**: versioned workloadがthreshold未達となりprofileでbounded hot pathが示された時だけ
  optimizeする。Boundary変更後は同じdistribution gateを再実行する。

## 7. Gap closure手順

Primary title failureは次の順で処理する。

1. Canonical checkpointからfirst divergenceを再現する。
2. State ownerと、leaf／gateway／feasibility blockerの分類を決める。
3. Windows trace、Rust model、controlled PE、Node runner、Chrome E2Eから最小oracleを得る。
4. 既存ownerへgeneral contractをfailure atomicに実装する。
5. Representative positive caseとinvariant-preserving negative caseを追加する。
6. Subsystem test、public fixture integration、関連primary／browser regressionを通す。
7. 現在値はmanifestだけ、stable behaviorは対応subsystem文書だけを更新する。

EXE／DLL／asset patch、EIP固有skip、title内部VM再実装、stateを作らないsilent successはgap closureと
認めない。

## 8. Deferred branch

- DirectDraw comparisonはactive targetの未知を直接減らし、bounded stopを定義できる場合だけ開く。
- Multi-titleはactive full-play完了後、新しいcorpusとacceptance scopeを明示してから開始する。
- Public release hardeningと追加browser tierは別のdistribution decisionを必要とする。
- x64と後世代DirectX／audio stackは独立したfeasibility roadmapを必要とする。

## 9. 完了とblocker

Active roadmapは、[requirements.md](requirements.md)のfull-play target、immutable source、
restart-safe save／load、clean stop／cancel、関連regression evidence、residual-risk記録をすべて満たした時だけ
完了する。

必要なoracle／corpusがなくcorrectnessを判定できない、必要behaviorがscope外または利用不能なplatform
capabilityに依存する、安全／authority invariantを守る設計が成立しない、または継続に新しい権限／重大な
scope判断が必要な場合は、structured blockerとして停止する。
