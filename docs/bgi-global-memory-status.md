# BGI GlobalMemoryStatus checkpoint

## Scope

このcheckpointは、primary BGIが到達したPE32／i386の
`KERNEL32.dll!GlobalMemoryStatus`一回だけを閉じる。32-byte
`MEMORYSTATUS` layout、VOID x86 stdcall、LastError preserve、consumerが読む
physical-memory fields、既存system-information policyとのauthority relationを対象とする。
`GlobalMemoryStatusEx`、hostの実memory pressure、allocator quota、動的なavailable値は対象外である。

## Native consumer evidence

Focused observationを同じsource executableから独立に二回取得した。各runはprimary caller
RVA `0x00081FAE`のcallを一回含み、32-byte outputの前後16-byte guardを保持した。
独立oracleはsentinel LastError `0`、`87`、`0x12345678`をすべて保存する。
両runで`dwTotalPhys`／`dwAvailPhys`は`0x7FFFFFFF`、page-file fieldsは
`0xFFFFFFFF`、`dwTotalVirtual`は`0x7FFE0000`で安定した。Memory loadと
`dwAvailVirtual`はhost状態により変化するためartifact observationであり、compatibility policyへ
importしない。VOID API後のEAX観測値もsemantic resultではない。

Caller continuationはstructure offset 8と12、すなわち`dwTotalPhys`と`dwAvailPhys`を順に読み、
各値が`0x80000000`以上なら`0x7FFFFFFF`へclampして同じ初期化helperへ渡す。他のfieldを
primary consumer contractとはしない。Raw trace／summaryは`.work/windows-traces/`に保持し、
source executable、DLL、assetは変更しない。Exact artifact identityは
[current-checkpoint.json](current-checkpoint.json)だけを正本とする。

## Runtime contract and authority

`Kernel32CompatibilityPolicy.memory_status`がmemory load、physical、page-file、available-virtual
fieldsを所有する。`dwTotalVirtual`は別のpolicy値として複製せず、既存
`Kernel32CompatibilityPolicy.system_info`のinclusive PE32 application spanから導出する。
Handler `kernel32.global-memory-status.x86-deterministic.v1`は全32 bytesのwritabilityをpreflightし、
一回のguest-memory commitでlittle-endian structureを書き、EAXとLastErrorを保存する。
NULL、cross-page、read-only output、disabled／malformed policyはguest bytes、generation、policy stateを
変更する前に具体的faultで停止する。

Policy profileはnativeのlegacy 32-bit saturation fieldsをbounded compatibility値として採用するが、
browser／host memoryをWindows stateへ取り込まない。このprofileはruntime heapやGuestMemoryのquota、
allocation admission、browser Wasm linear-memory capacityのauthorityではない。

## Integration acceptance

Controlled dispatcher testsはexact layout、callee cleanup、VOID／EAX preserve、LastError preserve、
guard保持、failure atomicityを検査する。Primary Node／Chromeはsequence 107,757、instruction
16,384,108、argument `0x006FF674`、return `0x10081FAE`の一completionと同じ32-byte output、
policy／virtual-span authority、host／browser memory non-importを比較する。Ordered semantic digestと
terminal memory digestを一致させ、次のstructured stopを
`KERNEL32.dll!GetFileAttributesA`とする。Current counts、hash、generation、browser artifactは
[current-checkpoint.json](current-checkpoint.json)だけを正とする。
