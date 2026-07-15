# COM ABI／IUnknown foundation

- ステータス: Implemented
- 対応: `P3-COM-01`、`REQ-COM-01`の共通foundation、`AC-P3-08`

## 境界

`runtime/com.rs::RuntimeCom`がlogical COM object、interface、IID、vtable method、共有reference
countを所有し、`RuntimeKernel`はprocess address allocationと`GuestMemory`をまたぐtransactionだけを
調整する。Win32 HANDLE用`RuntimeObjectTable`へCOM pointerやreference countを混在させない。

Object IDとinterface IDはRust-ownedの単調増加identityであり、guest pointer、host pointer、native
COM pointerのいずれでもない。各objectはcanonical `IID_IUnknown` interfaceを一つ持つ。
`QueryInterface(IID_IUnknown)`はどのinterfaceから呼んでもその同じpointerを返し、supported IIDは
stableな既存interface pointerを返す。成功時だけobject共有reference countを一つ増やし、
`E_NOINTERFACE`ではoutputをNULLにしてcountを変えない。NULL IID／output pointerは`E_POINTER`とする。

## x86 guest ABI

各objectはprocess allocatorの`com-endpoint` allocation IDに帰属する8 KiB backingを持つ。

- data page: interface pointerとvtable。read-only／non-executable
- endpoint page: 16-byte固定slot。read／execute、write不可
- endpoint: exact `UD2`の後にVine-owned marker、interface index、vtable slotを保持

Gatewayはx86 COM stdcall frameを`return address, this, explicit arguments...`としてchecked readする。
Return EIPとcallee cleanup後のESPをstate mutation前に計算し、`this`がendpoint所有interfaceと一致する
ことを検査する。`QueryInterface`はHRESULT、`AddRef`／`Release`はULONGをEAXへ返す。COM callは
threadのLastErrorを変更しない。

## Atomicityとlifetime

- installはguest mapping／protectionをmemory transactionへstageし、allocation planとCOM registration
  planをともにpreflightしてからcommitする。失敗時はowner、mapping、allocation ID、COM IDを残さない。
- `QueryInterface`のguest output writeとreference incrementは一つのtransaction境界で完了する。
- `AddRef`／non-final `Release`はshared object countだけを更新する。
- final `Release`は全interface／endpoint index、guest mapping、memory owner、process allocationを同時に
  退役させる。いずれかのpreflight／memory operationが失敗した場合、Rust identityとallocationを
  partial mutationしない。

未対応slotはsilent successにせず、interface、IID、slot、method、endpoint、guest callsite付きで
mutation前に構造化停止する。Malformed stack、return address、`this`、IID／output memoryも同様に
checked faultとなる。

## Browser resourceとの分離

`browser/runtime-worker/browser-resource-table.ts`はWeb API objectを
`browser-resource:<positive-safe-integer>` namespaceで所有する。Rustのu64 object／interface IDは
精度を失わないdecimal stringの非所有associationとしてだけ保持する。Browser resourceのlocal
releaseはCOM countを変更せず、Rust final-release completionを受けたadapterだけがobject associationを
まとめてretireする。Browser tableはCOM referenceを生成・解放しない。

## Verificationとnon-claims

```powershell
npm run check:com-foundation
```

Rust fixtureはpointer→vtable→UD2 endpoint、canonical IUnknown identity、supported／unknown IID、HRESULT、
stdcall cleanup、LastError preserve、shared count、final retirement、malformed call／install／release rollback、
具体的なunsupported診断を固定する。Node contractはBrowser IDの非再利用、immutable association、local
releaseとRust refcountの分離を固定する。このgateは`check:fast`にも含まれる。

Windows traceではselected `IDirect3D9` rootの`AddRef`／`Release`到達が既に記録されているが、本実装は
synthetic共通foundationだけである。`IDirect3D9`／device／resource、DirectInput、DirectSound、DirectDraw、
graphics／audio backend、`CoInitialize`、apartment、class factory、BSTR／VARIANT／automationは実装しない。
具体的interfaceはcorpus traceから別taskとしてこのfoundationへ接続する。
