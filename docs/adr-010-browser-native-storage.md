# ADR-010 Browser-native source mount and OPFS overlay

## 1. Status

- Status: Accepted
- Decision date: 2026-08-04
- Scope: Chrome production clientのsource mount、writable overlay、permission、durability、multi-context exclusion
- Decision: 利用者が選択したread-only directoryをFile System Access APIでmountし、handle catalogをIndexedDB、title/profile-scoped writable overlayをOPFS上のSQLiteへ置く
- Supersedes: [ADR-006](adr-006-vfs-local-server.md)のlocalhost production backend。Local serverはdeterministic oracleとmigration sourceとして残す
- Implemented: source stream hash登録、IndexedDB handle復元、Worker clone、Runtime Worker source read、OPFS SAH-pool SQLite、generation CAS、quota、idempotency、exclusive Web Lock、Worker kill rollback、response-loss recovery、Chrome process restart
- Not implemented: production picker UI、permission revoke recovery、primary 5.9 GiB mount acceptance、controlled guest write/restart/load、tombstone/delete semantics、registry semantics、export/import/migration UI

## 2. Decision

Production topologyは次をbaselineとする。

```text
Vine Client
|-- Source mount
|   `-- user-selected shared directory (read-only FileSystemDirectoryHandle)
`-- Writable overlay
    `-- OPFS
        |-- modified/new files and directories
        |-- tombstones
        |-- registry
        `-- metadata, leases, and idempotency ledgers
```

固定secure originから配信するVine Clientだけがこのstorageへ到達する。Directory pickerとpermission requestはmain threadの明示的なuser gestureで行う。選択した`FileSystemDirectoryHandle`はversioned IndexedDB catalogへ保存し、Runtime Workerから専用Storage Workerへstructured cloneする。Sourceに対するwrite permissionは要求しない。

Writable bytesはsource directoryへ書き戻さない。専用Storage WorkerがSQLite Wasmの`opfs-sahpool` VFSを使い、`/vine/v1/sqlite-sahpool`内のtitle/profile別databaseを開く。Runtime Workerは既存のversioned Rust host-service requestをStorage Workerへ転送し、Windows object stateを複製しない。

## 3. Authority boundary

| Datum | Authority | Non-owning adapter／projection |
|---|---|---|
| Windows path、mount resolution、file／directory identity、handle、cursor、share、LastError、staged write | Rust `RuntimeFiles` | Runtime Worker request metadata |
| Source capability、browser permission、real Unicode path segments | Browser main threadのmount catalog | IndexedDBに保存した`FileSystemDirectoryHandle` |
| Source object descriptor、Windows-932 path、length、SHA-256 | Fingerprint済みsource manifest | Browser mount descriptor |
| Source bytes | User-selected directory | Object IDとbounded rangeによるStorage Worker read |
| Committed overlay bytes、existence、generation、hash、quota、ledger | OPFS SQLite | Rust committed snapshot／pending commit |
| Tombstone、registry、overlay metadata | OPFS SQLite schema | Rust semanticsが到達後に使うtransaction boundary |
| Browser resource lifetime、Web Lock、OPFS／IndexedDB error | Browser Storage Worker | Versioned storage response |

TypeScriptはguest pointer、Win32 handle、COM pointer、LastError、Windows path lookup、dirty decisionを所有しない。Storage requestはtitle fingerprint、snapshot／profile、source／object／directory ID、range、generation、hashだけを受け、生のhost absolute pathを受けない。

## 4. Immutable source mount

Mount登録はmanifest内の全objectをstreaming SHA-256で検証する。各descriptorはWindows-932の`relativePathA`と、browser filesystemを辿るreal Unicode `pathSegments`を明示的に結び付ける。ASCII pathだけは機械的なbindingを許可し、それ以外は曖昧なhost decodingを行わない。

登録時はfile sizeと`lastModified`をhash前後で比較し、manifestのlength／SHA-256と一致した場合だけhandleとdescriptorを一つのIndexedDB transactionで公開する。Session開始と各range readではsize／`lastModified`を再検証し、変更を検出したsourceはmutation前に停止する。Timestampを保持した悪意ある外部書換えまで毎rangeのfull hashで検出するとはclaimしない。Executable／DLLのmanaged immutable snapshotと外部差替え拒否は、module import milestoneで別途固定する。

保存済みhandleのpermissionはsession開始前に`queryPermission()`で確認する。再許可が必要な場合、Storage Workerはpromptを出さず、main threadへuser gestureが必要なtyped failureを返す。Picker cancellation、permission denial、revoke中のread、directory move／deleteはproduction UI E2Eの未完了gateである。

## 5. Writable overlay

Schema version 1は次を一つのtitle/profile databaseへ置く。

- `overlay_files`: allowlisted file identity、Windows path、existence、generation、length、SHA-256、content
- `overlay_directories`: fileと別namespaceのdirectory identity、existence、generation
- `overlay_tombstones`: source／overlay deletion projection用のpathとgeneration
- `registry_keys`／`registry_values`: title-local typed registry tree
- `overlay_metadata`: schema／runtime migration metadataの拡張領域
- `commit_ledger`／`directory_commit_ledger`: response loss後のsame-key idempotency
- `profile_metadata`／`profile_lease`: title、profile、root、quota、monotonic session epoch

現在guest-visibleに接続済みなのはexisting file read、file commit、allowlisted directory createである。Tombstoneとregistry tableは同じfailure-atomic transaction boundaryを予約するが、delete、registry lookup／enumeration、migration semanticsを先取りしない。

新規databaseだけがfingerprint済みsourceからoptional seedを一度読み込める。既存databaseは再seedせず、source directoryへ逆同期しない。Source executable／DLLをoverlayがshadowする登録はRustで拒否し続ける。

## 6. Transaction, crash, and concurrency

SQLiteはrollback journal `DELETE`と`synchronous=FULL`を使う。一つのcommit transactionがactive lease、base generation、request-key identity、content hash、whole-profile quotaを検証し、content、generation、hash、ledgerを同時にpublishする。Rustはvalid success completion後だけcommitted generationを進める。Known quota、busy、stale、revoked leaseはconcrete Win32 failureへ戻し、unknown publication outcomeとmalformed identityはsession-fatalとする。

`opfs-sahpool`はpool内のSyncAccessHandleを占有する。このためversion 1はorigin全体のexclusive Web Lock `vine.browser-storage.opfs-sahpool.v1`を取得し、一度に一つのwritable Storage Workerだけを許可する。二番目のcontextは待機せずtyped lock failureになる。Multi-tab read/write concurrencyが実要件になった場合は、profile別pool、SAH-pool pause、またはWeb Locks対応VFSをevidence付きで比較する。

Controlled Chrome checkはtransaction中のWorker terminate後に旧generationだけが見えること、commit publish後のresponse lossとWorker terminate後に新generationが見え、same key retryが重複適用しないことを確認する。これはbackend durability proofであり、Runtime Workerがunknown outcomeを自動回復してguest save UIを完了したというclaimではない。

## 7. Quota, eviction, and recovery

OPFSとIndexedDBはorigin quotaおよびsite-data deletionの対象である。Clientは`navigator.storage.persisted()`、`persist()`、`estimate()`をpreflightし、結果をdiagnosticへ記録する。ただしpersistent grantを起動条件にしない。Canonical headless Chrome checkではgrantがfalseでもprocess restart後のhandleとoverlayは復元したが、これはeviction免除を意味しない。

Full-play acceptanceにはprofile export／import、backup、schema migration前snapshot、quota不足診断、site-data deletionからの説明可能なrecoveryを追加する。これらがない段階でOPFSを唯一のbackupと表現しない。

## 8. Verification

```powershell
npm run check:browser-storage
npm run check:runtime:browser-worker
```

前者はpinned Chrome for Testingを二つのprocessに分け、IndexedDB handle roundtrip、Worker clone、source range、OPFS SQLite restart、exclusive lock、rollback、idempotencyを検証する。後者はpublic PE32 fixtureの`CreateFileA -> GetFileSize -> ReadFile -> SetFilePointer -> ReadFile -> CloseHandle`をRuntime Workerからbrowser-native source adapterへ通し、既存localhost adapterもoracleとして回帰させる。

Current semantic evidenceとnon-claimsは[current-checkpoint.json](current-checkpoint.json)を正とする。
