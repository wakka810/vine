# BGI CreateDirectoryA checkpoint

## Scope

このcheckpointはprimary BGIが到達したPE32／i386の
`KERNEL32.dll!CreateDirectoryA`を、既存`RuntimeFiles` VFSとwritable-profile SQLiteの
durable directory transactionとして閉じる。Windows-932 path decoding、Rust-owned current-directory
relative resolution、allowlist済みで初期状態がabsentのdirectory、NULL security attributes、success時の
LastError preserve、duplicate／missing-parent／file-conflictのWin32 errorを対象とする。

任意directory作成、再帰作成、source mount変更、host filesystem探索、security descriptor、delete／rename、
directory enumerationは対象外である。Exact call count、instruction、generation、artifact hash、次のstopは
[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

## Native behavior and consumer

Focused oracleを同じsource executableから独立に二回取得した。各runはNULL security attributesで次の四caseを
順に検査する。

- absent childの作成はTRUEを返し、sentinel LastErrorを保存する。
- 同じpathの再作成はFALSE／`ERROR_ALREADY_EXISTS` 183を返す。
- absent parent配下はFALSE／`ERROR_PATH_NOT_FOUND` 3を返す。
- existing regular fileとの衝突はFALSE／`ERROR_ALREADY_EXISTS` 183を返す。

Oracleが作成したdirectoryはrun内で削除し、source executable、DLL、assetは変更していない。Primary callerは
return RVA `0x000825E8`へ戻り、BOOL resultを成功判定に使う。Native host path、filesystem object ID、timestamp、
security descriptorはcompatibility policyへ取り込まない。Raw traceとsummaryはignored
`.work/windows-traces/`に保持する。

## Runtime and durability authority

`runtime/files.rs::RuntimeFiles`がnormalized path、directory ID、existence、generation、pending request、
mutation-free create planをprivateに所有する。Writable-profile snapshot version 3はfile objectとは別namespaceで
allowlisted directoryを登録する。Allowlistは作成可能性であって存在ではなく、initially absent directoryは
SQLiteとRustの両方でcommittedになるまで`GetFileAttributesA`から見えない。

Handler `kernel32.create-directory-a.runtime-vfs.v1`は二argument x86 stdcallである。Rustはpath、policy、
security argument、parent、file conflict、lifecycle、pending identityをpreflightし、owner planを作ってthreadを
host waitへ移す。Workerは`storage.profile-directory-create.v1`へtitle／profile／session／epoch／request／directory／
base generationを渡すが、guest pointer、Win32 handle、任意host pathは渡さない。

SQLiteはleaseを同じ`BEGIN IMMEDIATE` transactionで再検証し、directory generation compare-and-swap、
idempotency ledger、publicationを一括commitする。Rustはidentityとgenerationを再検証したsuccessful completionの後だけ
owner planをtargeted commitする。Storage failure、malformed／stale／duplicate completion、cancel、process exitは
Rust directory state、allocator、guest bytes、LastError、SQLite generationを部分更新しない。Successful createは
LastErrorを保存し、guest memoryへ書かない。

## Browser protocol and acceptance

Worker protocol version 24はversion 23のtyped file-attribute snapshotを継承し、directory-create request／completionと
profile snapshot version 3を追加する。TypeScriptはSQLite resourceとtransportを所有するだけで、Windows path、directory
identity、existence、generation、pending Win32 requestの第二の正本を作らない。Private runは共有profileを変更しない
run-local SQLite databaseを使い、`UserData`が毎回absentから始まることを固定する。

Rust model／dispatcher testsはsuccess publish ordering、duplicate、missing parent、file conflict、wrong owner、stale／
duplicate plan、capacity、malformed completion、host failure、cancellation、LastError preserveを検査する。SQLite／Worker testsは
schema migration、directory snapshot、lease、idempotent replay、generation conflict、rollback、terminal drainを検査する。
NodeとChromeは同じ一completionについてrequest／directory／generation identity、BOOL／LastError、zero guest writes、
source不変、ordered semantic digest、terminal memory digestを一致させる。

次のstructured stopは、既存の初回activation成功後に同じlogical windowへ到達した
`USER32.dll!SetWindowPos` repeat activation variantである。これはCreateDirectoryAのfailureではなく、別の
User32 semantic leafとして扱う。

## Non-claims

- Directoryはprofile manifestで明示allowlistされた一件だけで、general filesystem APIではない。
- Browser／Node host directoryを作成せず、distribution treeへ書き込まない。
- Directory handle、enumeration、current-directory変更、delete／rename、recursive create、ACLは未実装である。
- Primary save UI、browser restart後load、全scenario、long-run quotaはこのcheckpointだけではacceptance済みとしない。
