# BGI GetFileAttributesA checkpoint

## Scope

このcheckpointはprimary BGIが到達したPE32／i386の
`KERNEL32.dll!GetFileAttributesA`を、既存`RuntimeFiles` VFSのread-only path queryとして閉じる。
Windows-932 path decoding、current-directory relative resolution、登録済みsource／writable file、登録pathから
導出したdirectory、`INVALID_FILE_ATTRIBUTES`とnot-found LastError、success時LastError preserveを対象とする。
Host filesystem探索、wildcard、reparse point、volume metadata、general DOS attributes、directory作成／削除は対象外である。

## Native consumer evidence

Focused observationを同じsource executableから独立に二回取得した。各runは111 callsを含み、最初の
`debug.inf` probeはcaller return RVA `0x00061C80`で`0xFFFFFFFF`／`ERROR_FILE_NOT_FOUND` 2、
後続search-root probeはRVA `0x00061CDD`で`ERROR_PATH_NOT_FOUND` 3となる。`UserData` directoryは
RVA `0x00082648`で`0x30`、独立oracleのregular fileは`0x20`、current directoryは`0x30`、missing fileは
`0xFFFFFFFF`／2である。Successful queriesはsentinel LastErrorを保存する。

Regular-file callerはresultを4 bit右shiftして反転し、archive bitが立たない場合だけfile-present branchへ進む。
Directory callerはまず`0xFFFFFFFF`を比較し、その後directory bit `0x10`を検査する。したがってsemantic
contractはexact native handleやhost pathではなく、invalid-result判定、archive／directory bit、missing-file／
missing-path error classである。Raw trace／summaryは`.work/windows-traces/`に保持し、source executable、DLL、assetは
変更しない。Exact artifact identityは[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

## Runtime contract and authority

`RuntimeFiles`がnormalized path、mount、source／writable registration、committed existenceを所有する。
`resolve_path_kind`はimmutable borrowだけでexact registered existing fileをregular file、登録pathのancestorとmount rootを
directory、最終componentだけがない場合をmissing file、parent pathがない場合またはmount外をmissing pathへ分類する。
Browser／Node host filesystemへ問い合わせず、TypeScript側にWindows path stateを複製しない。

`Kernel32CompatibilityPolicy.get_file_attributes_a`はregular `0x20`とdirectory `0x30`だけを供給する。
Handler `kernel32.get-file-attributes-a.runtime-vfs.v1`はone-argument x86 stdcallで、success時はattributeを返して
LastErrorを保存し、missing fileは`0xFFFFFFFF`／2、missing pathは`0xFFFFFFFF`／3を返す。NULL、malformed、
mount escape、disabled／malformed policyはVFS、allocator、guest memory、LastErrorを部分更新する前に拒否する。
Queryはfile、directory、handle、browser resourceを作成しない。

## Integration acceptance

Rust model／dispatcher testsはcase-insensitive Windows-932 resolution、registered source／existing writable file、derived
directory、missing final／parent、mount escape、success LastError preserve、disabled policy、state snapshot不変を検査する。
Primary Node／Chromeは23 completionsを同じsequenceで比較し、3件をmissing file、20件をmissing pathとして扱う。
First callはsequence 107,788／instruction 16,396,079、last callはsequence 108,205／instruction 16,445,360で、
いずれもguest write zeroである。Ordered semantic digest、terminal memory digest、path authority、host／browser filesystem
non-importを一致させ、次のstructured stopを`KERNEL32.dll!CreateDirectoryA`とする。Current counts、hash、generation、
browser artifactは[current-checkpoint.json](current-checkpoint.json)だけを正とする。
