# ADR-006 Local VFS and persistence backend

## 1. Status

- Status: Superseded for the production backend by [ADR-010](adr-010-browser-native-storage.md)
- Decision date: 2026-07-16
- Scope: 最初のasset／save vertical sliceに使ったstorage topologyとauthority境界
- Historical decision: same-origin localhost Vine serverを最初のbackendとし、immutable sourceはread-only、save／registryはtitle-local SQLite transactionへ保存する
- Implemented: bounded immutable-source manifestとwritable profile、Windows-932 path／share／cursor／typed-handle／staged state、`CreateFileA`／`GetFileSize`／`ReadFile`／`WriteFile`／`SetFilePointer`／`FlushFileBuffers`／`CloseHandle`、same-origin range／SQLite commit service、generation／idempotency／quota／rollback、cancel／watchdog cleanup
- Not implemented: controlled browser save／restart／load acceptance、kill matrix、registry projection、migration／backup UI、general Windows file breadth

このADRはdeterministicな初期backendとoracleを記録する履歴であり、VFSまたはpersistenceの完成を意味しない。2026-08-04以降のproduction baselineはFile System Access API、IndexedDB mount catalog、OPFS overlayを採用したADR-010を正とする。Local server／SQLite adapterはfixture、native比較、failure injection、将来のmigration sourceとして維持する。

## 2. Context

Windows file APIはguestから見ると同期的だが、browser storageとnetwork requestは非同期である。Windows path、handle、cursor、sharing、LastError、overlay resolutionと、host上のdurable bytesを同じ層へ置くと、RustとTypeScript／serverに二つのWindows state authorityが生じる。

最初からbrowser内の永続化だけを採用すると、Windows semantics、OPFS／IndexedDB lifecycle、quota／eviction、restart recoveryを同時に解く必要がある。個人利用のactive targetでは、UIとAPIをlocalhostの同一originから提供し、source fileへread-onlyで到達できるlocal companion serverの方が短いvertical sliceとfailure injectionを作りやすい。

## 3. Accepted boundary

| State | Authority | Adapter／mirror |
|---|---|---|
| Windows path、mount resolution、file／directory identity、handle、cursor、access／share mode、LastError | Rust VFS subsystem | なし |
| Immutable source snapshot／object descriptor、Windows path index、open identity | Rust VFS subsystem | Local server registration |
| Fingerprint済みsource bytes、complete-file length／hash verification、range delivery | Local Vine server | Rust logical source object／pending request |
| Committed save／registry bytes、generation、schema、content hash | Local Vine serverのtitle-local SQLite | Rust logical transaction／file state |
| Async request、cancel、timeout、completion delivery | Runtime Worker | Rust scheduler-owned host request |

Runtime Workerとserverはguest pointer、Win32 handle、生のhost absolute pathをauthorityとして保持しない。RustはWindows pathを正規化してmount内のstable object IDへ解決する。Source read transportはsource／object IDとrangeだけをWorkerへ渡し、Workerはprocess準備時にRustと共有したimmutable manifestへtitle fingerprint／snapshot IDをbindingしてserver requestを作る。Profile transportはreserved source ID zero、profile／object ID、generation、range、またはbase generation／length／hashを渡す。WorkerはRust-owned staged payloadを検証して運ぶだけで、file stateを所有しない。

Local serverはVine UI、immutable source API、save APIを同じloopback originから提供する。任意directoryをHTTP越しに公開せず、利用者が明示的に登録したread-only source rootと、Vine管理のwritable profile rootだけへアクセスする。Remote upload、multi-user同期、cloud accountは初期scope外とする。

## 4. Source and save model

Source executable、DLL、assetはfingerprint済みsnapshotとしてread-onlyに扱う。Large assetは必要rangeをlazy readできるが、module mappingと再起動に使うidentity／hashは安定させる。Writable overlayがsource executable／DLLをshadowしてはならない。

Saveとvirtual registryはsourceと別namespace、別quota、title fingerprint別SQLite databaseへ置く。最初のtransaction contractは次とする。

1. Open時にcommitted generationを取得し、Write／Seek／Truncateはそのgenerationに対するstaged write-setへ適用する。
2. Flushまたは成功したCloseをdurability boundaryとし、SQLite transactionでbytes、metadata、generation、content hashを一括commitする。
3. Base generationがstaleなcommit、重複request、wrong title／profile、quota超過はcommit前に拒否する。
4. Cancel、watchdog、server／Worker killでは未commit write-setを破棄し、最後のcommitted generationを残す。停止を理由に自動commitしない。
5. Guestへ返すWrite／Flush／Closeの厳密なWindows-visible成功時点はnative caller evidenceで固定する。Write-through等の未到達variantは推測で一般化しない。

SQLiteは最初のtransactional storeであり、Windows file semanticsのauthorityではない。Large writable fileやmulti-file atomic groupが実corpusで必要になった場合は、content-addressed object＋SQLite metadata等を同じcommit contractの下で比較する。

## 5. First vertical slice

最初のgateはAPI数ではなく、次の一経路で閉じる。

1. Fingerprint済みtitleが実際に読むassetを一つ選び、Rust VFSのOpen／Read／Seek／Closeからlocalhost source APIまで通す。
2. Source bytesと実行前後のEXE／DLL／asset hashが一致することを確認する。
3. 一つのsave fileをCreate／Write／Seek／Closeし、durable generationとhashを記録する。
4. Runtime Worker、browser context、local serverを停止して再起動し、Open／Read／load後のbytesとstate hashを比較する。
5. Commit前、transaction中、publish後へkillを注入し、再起動後に旧generationまたは新generationの完全な一方だけが見えることを確認する。
6. Wrong path／title／generation、quota full、duplicate completionをmutation前に拒否する。

### 5.1 Implemented storage foundation

Source halfはpublic PE32 fixtureとprivate targetで完了した。Rust `RuntimeFiles`がmanifest、Windows-932-aware path identity、typed file payload、share、cursor、pending requestを所有し、Workerはsame-origin byte requestだけを行う。Local serverは明示登録fileの全bytes／SHA-256を先に検証してimmutable copyからrangeを返す。HTTP identityはtitle fingerprint、snapshot、source、object、offset、requested／transferred lengthを往復検査する。

Public fixtureはOpen／Size／Read／Seek／Read／Closeを自己検証し、Chromeはsuccess、pending cancel、watchdogを通す。Writable profileはRustがcreate／truncate／write／seek／dirty read／flush／closeをplan／commitし、SQLiteがgeneration CAS、content hash、quota、idempotency ledgerを一transactionで保持する。Modelはrestart、rollback、commit後response lossとfailed Close retryを通す。Private targetはBGI.gdbをprofileとして読み、Node／Chrome semantic equivalenceと配布copy不変を確認する。詳細contractは[runtime-vfs.md](runtime-vfs.md)を正とする。

Controlled guestによるChrome commit、server／browser restart後load、kill injection、registry／multi-file lifecycleは未完了であり、このfoundationからprimary save scenario完了をclaimしない。

## 6. Consequences and reconsideration

初期実装はbrowser storage固有問題をVFS semanticsから分離でき、server restartとSQLite transactionへ決定的にfailureを注入できた。このlocalhost backend単独では、Vine local serverが起動していない状態でのoffline browser-only実行をclaimしない。

OPFS／IndexedDBへの再検討は[ADR-010](adr-010-browser-native-storage.md)で完了した。Remote save同期、複数device、multi-user conflictはactive scope外に残る。Backendが変わってもRust VFS authority、source／overlay分離、generation付きdurable commitは変更しない。
