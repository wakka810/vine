# Runtime identity registries

## Purpose

reviewの`RuntimeState` responsibility splitとして、process-wide dynamic endpoint identityと
encoded-pointer identityをprivate `runtime/registries.rs::RuntimeRegistries`へ移した。Endpoint table、
token table、source index、pointer policy、monotonic token allocatorをaggregate rootの個別fieldとして
扱わず、同じownerのvalidation／plan／commitを通す。

このcheckpointはWin32 capabilityを追加しない。Existing 46 contracts／284 calls、dynamic FLS dispatch、
TLS endpoint classification、`EncodePointer`／`DecodePointer`のguest-visible relationをそのまま維持する。

## Ownership boundary

`RuntimeRegistries`だけが次を所有する。

- dynamic endpoint IDからimmutable endpoint recordへのtable
- encoded tokenからsource recordへのtable
- sourceからtokenへのinverse index
- encoded-pointer namespace／per-process cookie policy
- next encoded-pointer ID（token derivationではなくdiagnostic registration order）

全collection fieldはprivateであり、module外のinspectionは`dynamic_endpoints()`／
`encoded_pointers()`のimmutable borrowに限定する。`RuntimeState`はlookupをdelegateし、mapへ
`insert`／`get`／iterationを行わない。

Serialized `registries.dynamicEndpoints`／`registries.encodedPointers` shapeは維持する。Root
`encodedPointerNamespace`／`encodedPointerCookie`／`encodedPointerSources`／
`nextEncodedPointerId`もborrowed compatibility viewであり、aggregateの第二authorityではない。
`registries.hostRequests`は同じJSON objectに現れるが、private `RuntimeScheduler`から借用する
compatibility fieldであり、このregistry subsystemは所有もmutationもしない。

## Dynamic endpoint transaction

Aggregateはprocess layoutのdynamic-export rangeとowned endpoint valueをsubsystemへ渡す。
`plan_dynamic_endpoint`は次をcollection mutation前に検査する。

- controlled namespace、nonzero ID、module／name／lifetime identity
- IDとslot sizeから導くaddressがprocess dynamic-export range内であること
- endpointがそのexact slot addressを持つこと
- live IDまたはaddressとのcollisionがないこと

Plan成功後の`commit_dynamic_endpoint`だけがone recordを挿入する。Duplicate ID／address、invalid slot、
arena exhaustionはtableを不変にする。Aggregateはaddress-space ownershipとregistry ownershipを
coordinatorとして接続するだけである。

## Encoded-pointer transaction

`plan_encode_pointer(source)`はexisting sourceならstable token relationをreverse-validationし、新規sourceなら
Windowsと同じcookie transform `ror(source ^ cookie, cookie & 0x1f)`でtokenを導く。Cookieはprocessごとの
bootstrap valueであり、`cookie & 0x1f != 0`をpolicyとして要求してrotationを必ず効かせる。Transformは
u32上のbijectionなので、distinct sourceがtokenを共有せず、同じsourceはprocess lifetime中stable tokenを保つ。
Token collisionとID exhaustionはtoken table、source index、allocatorのいずれも変更しない。

`commit_encode_pointer`はnew record、inverse index、next IDを一つのinfallible targeted commitで進める。
Same sourceは同じtokenを返してstateを変えない。`decode_pointer`はcookie inverseがrecordのsourceと一致する
registered tokenだけをsourceへ戻す。Token IDはregistration orderのdiagnosticであり、tokenの導出には
関与しない。Host pointer値はauthorityへ入らない。

## Contract and regression

Representative contract:

```powershell
cargo test -p vine-core --lib runtime::registries::tests --locked
cargo test -p vine-core --test runtime_registries --locked
cargo test -p vine-core --test api_dispatcher pointer_encoding_contract_round_trips_only_process_owned_tokens --locked
cargo test -p vine-core --test api_dispatcher runtime_state_drives_five_api_shapes_and_nested_scheduler_state --locked
```

Subsystem tests fix mutation-free endpoint／pointer plans、all-index commit、identity reuse、collision／exhaustion
rejection and standalone serialization shape. Aggregate tests fix duplicate endpoint rollback、dynamic
endpoint-to-thread TLS classification、pointer identity reuse、invalid decode rollback and the complete existing
runtime JSON compatibility view. PE probe reads endpoint count through the immutable API.

Current Node／Chrome result、artifact identity、benchmark、next structured stopは
[current checkpoint manifest](current-checkpoint.json)だけを正とする。

## Non-claims and next dependency

`decode_pointer`はregistered tokenだけを受け付ける。Windowsの`DecodePointer`は未登録tokenにもcookie inverseを
返すが、Vineはguest-visible stateのauthority modelを保つためregistry gateを維持し、未登録tokenは
`encoded-pointer-token-unknown`で停止する。65,535 recordのtoken space boundも同じ理由で維持する。

Dynamic endpoint IDのlifetime retirement、FLS callback teardown、snapshot-stable identity、COM vtable endpoint、
general executable arena allocationは未実装である。Existing fixed dynamic-export rangeはprocess address-space
allocator registryへ登録したversion 1 preferred layoutであり、このcheckpointは新しいfixed guest addressを
追加しない。Runtime responsibility checkpoint S13 completes the aggregate ownership split; the saved
`GetCurrentThread` stop remains the next capability checkpoint.
