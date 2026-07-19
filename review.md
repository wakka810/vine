## P0

* **問題点：主要サブシステムの成立性が未検証**

  * **現状：** CPU実行、PEロード、Win32 APIの初期化・起動経路は前進しているが、Graphics、Audio、Input、保存・再起動、Thread／Wait／Timeout／Exceptionなど、フルプレイに直結する本番経路は未成熟。
  * **推奨：** 個別APIの追加より先に、first present、連続音声、入力反映、save／restart／load、複数スレッド実行など、各サブシステムの最小実動経路を成立性ゲートとして検証する。

* **問題点：開発進捗がAPI到達数に偏っている**

  * **現状：** 未実装APIを順次追加して起動地点を前進させているが、ゲームとして表示・操作・再生・保存できる状態には至っていない。
  * **推奨：** API数ではなく、起動、表示、入力、音声、保存、再開などのユーザーシナリオ単位で進捗と完了条件を管理する。

* **問題点：D3D9の必要対応範囲が未確定**

  * **現状：** `CreateDevice`や`D3DXCreateEffect`への到達は確認されているが、実タイトルが必要とする描画機能、state、resource、shaderの範囲が明確になっていない。
  * **推奨：** 実タイトルの呼び出しと描画経路を追加観測し、対応対象、非対応対象、代替実装の範囲を早期に固定する。

* **問題点：Rust–TypeScript／Wasm間の通信コストが将来のボトルネックになり得る**

  * **現状：** 通常処理でもJSON変換、デコード、メモリコピーが発生しており、描画、音声、asset streamingなど高頻度処理の増加に耐えられるか未確認。
  * **推奨：** 高頻度経路には軽量なバイナリ表現、共有メモリ、batch処理などを導入し、詳細なJSONは診断・証跡用途に限定する。

* **問題点：API・capability・checkpoint定義が重複管理されている**

  * **現状：** API識別子、schema、policy、期待値、manifest、Rust、TypeScript、validator、テスト、文書に同種の情報が個別記述されている。
  * **推奨：** versioned registryまたは共通contract catalogを唯一の定義元とし、dispatcher、型定義、manifest、validator、テスト雛形、文書を生成・検証する構造へ移行する。

* **問題点：checkpointとevidenceの構造が肥大化している**

  * **現状：** 現在状態、構造定義、期待値、成果物hash、benchmark、停止点などが大規模なJSONや検証スクリプトに混在している。
  * **推奨：** 現在状態、capability一覧、個別checkpoint、期待値、evidence index、成果物metadataを分離し、生成可能な部分をデータ駆動化する。

* **問題点：証跡データのサイズと生成コストが過大**

  * **現状：** 起動途中でもsemantic reportや完全traceが数百MB規模になり、保存、転送、比較、更新のコストが増大している。
  * **推奨：** 通常実行では集約結果、重要イベント、失敗周辺、識別hashのみを保存し、完全traceは明示的な診断モードや障害解析時に限定する。

* **問題点：証跡契約が内部実装に過剰結合している**

  * **現状：** フィールド名、内部型、テスト名、ファイル構造などがcheckpointの合否条件に含まれ、内部リファクタリングだけでも証跡更新が必要になる。
  * **推奨：** acceptance evidenceを外部挙動、observable state、invariant中心に再定義し、内部構造の検査は別のstructural testとして分離する。

* **問題点：永続化の製品経路が未完成**

  * **現状：** read-only file accessは成立しているが、書き込み、flush／close、SQLite commit、障害復旧、再起動後のloadが未実装または未検証。
  * **推奨：** 実タイトルのsave一件を対象に、write、commit、close、restart、loadまでを早期に縦断し、独立した重要ゲートとして管理する。

* **問題点：Thread／Wait／Timeout／Exceptionの一般経路が未完成**

  * **現状：** 複数CPU contextの基礎はあるが、一般的なthread生成、同期、wait、timeout、例外伝播・復帰の挙動が不足している。
  * **推奨：** 実タイトルで必要なthread behaviorを観測し、最小の複数thread fixtureと例外fixtureでscheduler、同期、timeout、exception contractを先行検証する。

## P1

* **問題点：`vine-core`と`RuntimeKernel`に責務が集中している**

  * **現状：** メモリ、process、thread、scheduler、USER32、GDI、COM、VFS、file、heapなど多数のsubsystem調整、API処理、serialization、diagnostic生成を単一クレートまたはaggregateが担っている。
  * **推奨：** execution、kernel、Win32、host I/Oなどの境界を明確にし、状態所有権を維持しながら、用途別command、transaction、query、diagnostic生成を外部コンポーネントへ分離する。

* **問題点：公開APIが内部構造を広く露出している**

  * **現状：** crate rootやfacadeからsubsystem record、owner、内部型、定数が多数再exportされ、内部変更の影響が広範囲へ波及しやすい。
  * **推奨：** 公開面をcommand、query、transaction、opaque ID、error、versioned snapshot／viewなど安定した境界に限定し、subsystem内部型を閉じる。

* **問題点：実行・検証ハーネスが巨大化している**

  * **現状：** probe、JavaScript／Node runner、Browser Worker、Windows trace agent、checkpoint validatorに、実行制御、通信、profile、scenario、host request、watchdog、検証、証跡生成が混在している。
  * **推奨：** 共通session基盤、実行制御、host service、通信、watchdog、evidence収集、report生成、個別scenario／checkpointを責務単位で分割する。

* **問題点：Browser Workerのsession classが肥大化している**

  * **現状：** execution loop、thread context、host request、source read、watchdog、report生成を単一classが管理している。
  * **推奨：** execution controller、thread manager、host service、watchdog、artifact collectorを独立コンポーネントとして切り出す。

* **問題点：Wasm bridgeにsession状態と対象タイトル固有処理が混在している**

  * **現状：** global bridge state、単一session前提、対象タイトル固有hashやprofile、ABI、実行制御がruntime実装内で密結合している。
  * **推奨：** 汎用bridgeとtarget profileを分離し、session identity、target configuration、capabilityを明示的な入力として扱う境界へ整理する。

* **問題点：v86の内部実装への依存が残っている**

  * **現状：** private offsetや内部値、保守的な命令fetch判定など、v86の非公開構造を上位層が参照している。
  * **推奨：** 依存箇所をadapterの正式な境界へ閉じ込め、v86更新時の影響を局所化する。

* **問題点：x86命令fetchの境界契約が不完全**

  * **現状：** page境界付近で固定長のfetch範囲を事前検査するため、安全側ではあるものの、実際には実行可能な短い命令を拒否する可能性がある。
  * **推奨：** decoderが実際に消費する命令長に基づいてexecute permissionを判定し、page boundary、fault、partial decodeの契約を明文化する。

* **問題点：benchmarkが実ゲーム負荷を代表していない**

  * **現状：** 起動sessionの反復が中心で、描画、音声、asset streaming、入力、save処理などの継続負荷を含んでいない。
  * **推奨：** representative sceneや実タイトルの区間を用い、frame、audio buffer、streaming、saveを含むシナリオbenchmarkを追加する。

* **問題点：テストコード自体の保守コストが高い**

  * **現状：** API dispatcher、USER32、checkpoint検証などのテストが数千行規模になり、fixture setupや期待値の重複が増えている。
  * **推奨：** API名ではなく、pointer validation、callback、lifecycle、atomicity、state transitionなどの振る舞い単位に分割し、fixture builder、table-driven test、state-machine test、property testを導入する。

* **問題点：fuzz対象が限定的**

  * **現状：** PE loader以外のprotocol、marshaller、path、encoding、schema、host requestなど、外部入力を解釈する境界のfuzzが不足している。
  * **推奨：** 入力境界を優先してfuzz targetを追加し、短時間のCI実行と長時間の定期実行に分けて運用する。

* **問題点：CIが単一の長いジョブに集中している**

  * **現状：** 文書・semantic check、Rust test、Node、fixture E2E、Chrome、benchmarkなどが直列または同一ジョブで実行され、feedbackが遅く、失敗箇所の切り分けと並列化が難しい。
  * **推奨：** static／semantic、Rust、fixture、browser E2E、performance observation、Windows oracleを独立jobに分割し、必要なものを個別のrequired checkとして管理する。

* **問題点：Chrome stableへの依存により再現性が低下する**

  * **現状：** semantic testやbrowser E2Eが自動更新されるChrome stableの影響を受ける。
  * **推奨：** acceptance用の固定revisionと、最新stableに対する互換性観測を別ジョブとして運用する。

* **問題点：継続的なWindows native検証が不足している**

  * **現状：** Linux CIと記録済みWindows証跡が中心で、native trace取得経路やWindows固有挙動を継続的に検証できていない。
  * **推奨：** 定期実行または変更条件付きのWindows CI／oracle jobを追加し、native traceとbrowser側結果の差分を継続監視する。

* **問題点：文書とcheckpoint情報の維持負荷が高い**

  * **現状：** API単位の定型文書が増加し、実装、registry、manifest、テスト、evidenceとの同期コストが高くなっている。
  * **推奨：** API一覧、schema、対応状況などの定型情報は共通registryから生成し、手書き文書は設計判断、invariant、risk、非対応範囲、acceptance criteriaに限定する。

## 調査・対応記録

### P0-01 主要サブシステムの成立性

* **判定：** 対応済み（進捗表現の欠陥を修正）。
* **調査結果：** Primary targetは論理window初期化の途中であり、first present、実input、実audio、save／restart／load、一般thread／exceptionをまだ成立済みと主張できない。一方、controlled Browser Worker fixtureはpixel、880 Hz pulse、persistent byte、複数CPU context、nested callback、host wait／cancel／watchdogを通しており、execution feasibility自体が全く未検証という状態ではない。問題は、このfixture-only evidenceとprimary product pathの区別がCurrent Statusから読み取りにくかったことである。
* **変更内容：** `docs/current-checkpoint.json` schema 3へ9個のscenario milestone、active milestone、status、evidence boundaryを追加した。`fixture-only`／`bounded-foundation`／`not-reached`を`in-progress`から分離し、READMEを同じmanifestからscenario-firstで生成するようにした。
* **検証結果：** `node tools/test-current-checkpoint.mjs`と`npm run check:docs`でschema、canonical order、active milestone、README生成を検証した。Private-target gateのcontrolled fixtureはpixel、audio、persistent byte、source read、thread／callback、cancel、watchdogを一つのbrowser runで検査する。
* **未対応理由：** Primaryの未到達product pathをこのレビューだけで完成させない。これらは`docs/roadmap.md`のsubsystem gatewayと`docs/risks-and-open-questions.md`の`Q-THREADS`／`Q-SAVE`／`Q-GRAPHICS`／`Q-AUDIO`に明示されたactive workであり、今回の対応は未検証を合格扱いしない状態管理までとする。

### P0-02 API到達数に偏った進捗表示

* **判定：** 解決済み。
* **調査結果：** READMEの先頭表は`17,841 calls across 83 API contracts`を`Active runtime`として強く表示し、user-visible scenarioの現在地を表示していなかった。Manifestにもscenario単位のmachine-readable statusがなかった。
* **変更内容：** READMEの主表示を`Active scenario`へ変更し、API call／contract数は`Runtime diagnostics`へ降格した。boot、window、asset、present、input、audio、save、representative flow、full-playを固定順で生成するtableを追加した。
* **検証結果：** `tools/test-current-checkpoint.mjs`が全milestone ID／statusの生成、invalid status、duplicate／out-of-order IDを検査し、`npm run check:docs`がgenerated blockのdriftを拒否する。
* **未対応理由：** なし。API数は次stopのdiagnosticとして残すが、進捗の完了判定には使用しない。

### P0-03 D3D9の必要対応範囲

* **判定：** 根拠付き対応不要（現在のgatewayまで既にbounded）。
* **調査結果：** Native vtable evidenceは`IDirect3D9` root 11 unique／53 total callsと、BGI RVA `0x5ACA5`からの`CreateDevice` 3 calls（Adapter `0`、DeviceType `1`、BehaviorFlags `0x44`、HRESULT `0`）を固定している。Device／resource／state／draw／Present／Resetは未観測であり、実装済みとは扱われていない。`docs/d3d9-frontend.md`はroot queryの対応範囲とnon-claimsを列挙し、version別D3D frontendの下にWineD3D相当の`RuntimeGraphics` logical device／resource／command boundaryを置き、最初のstate ownerが必要になる`CreateDevice`から開始すると定めている。
* **変更内容：** 新しいgraphics frameworkは追加していない。Scenario manifestのfirst-presentを`fixture-only`とし、primaryのCreateDevice／draw／Presentが未到達であることをCurrent Statusへ明示した。
* **検証結果：** `cargo test -p vine-core --test d3d9_dispatcher --locked`がroot COM identity、policy、output、negative path、failure atomicityを検査する。Native evidenceとcurrent manifestは`check:docs`／checkpoint validationで参照整合を検査する。
* **未対応理由：** 未観測のdevice methodを推測でcatalog化することは、到達behaviorだけを扱うscope規則と未到達frameworkを作らない原則に反する。追加device traceとsupport band確定は`CreateDevice` subsystem gatewayのexit gateで行い、D3D10／11／DXGIはdeferredのままとする。

### P0-04 Rust／TypeScript／Wasm間の通信コスト

* **判定：** 根拠付き対応不要（指摘された通常payload経路は既にbinaryで、性能blocker未再現）。
* **調査結果：** Guest memoryのfull reflection、incremental delta、automatic writeback、host deltaはversioned binary control／payload bufferを使用し、JSONはbounded control metadataとdiagnostic／evidenceに限定されている。BGI browser benchmarkは同じ11,807,189-instruction workloadを複数session実行し、現在のthresholdを満たしている。SharedArrayBufferは実測上必要になるまでoptionalとする`Q-PLATFORM`方針である。
* **変更内容：** Transport変更は行っていない。P0-07で通常semantic evidenceをsummary＋digestへ変更し、JSON reportのI/Oコストは別途除去済みである。
* **検証結果：** `npm run check:v86-benchmark-gate`がversioned threshold contractを検証し、private browser benchmark evidenceは29 sessionsの`threshold-passed`を保持している。Binary delta／writebackのtruncation、stale generation、wrong owner、no-partial-changeはfixture E2EとRust testsの対象である。
* **未対応理由：** 測定済みthreshold failureやprofile上のhot pathがないため、共有memory必須化や追加batchingは推測最適化になる。描画／音声／streamingのrepresentative workloadが到達してfailureを示した時点で同じversioned gateから再検討する。

### P0-05 API・capability・checkpoint定義の重複管理

* **判定：** 根拠付き対応不要（runtime semanticsの複数authorityは再現しない）。
* **調査結果：** Guest-visible ABI／behaviorはRustの`ApiContract`とmodule別`*_contract_for_endpoint`が生成し、`ApiDispatcher`がendpoint identity、calling convention、pointer constraintを検証する。Wasm bridgeは同じcontractを収集してdispatchし、TypeScript WorkerはWindows API schemaやhandlerを複製していない。ManifestのAPI一覧／countとNode／Chrome profile値はruntime authorityではなく、到達evidenceを独立照合するoracleである。Controlled fixture固有contractはproduct API catalogと別scopeである。
* **変更内容：** 共通registryやcode generationへの移行は行っていない。Scenario statusだけを新しい単一manifest groupに追加し、README表示とvalidatorをそこから生成した。
* **検証結果：** `npm run check:checkpoint-manifest`、`npm run check:private-target-contract`、Rust dispatcher testsが、contract count／endpoint／ABIとevidenceの不一致を拒否する。実行時のcontract selectionは`crates/vine-runtime-wasm`からRust catalog関数を直接呼ぶ経路であることをcode inspectionで確認した。
* **未対応理由：** 独立oracleやhistorical evidenceまで同じgeneratorへ統合すると、誤りを同じ定義から自己検証する構造になる。三箇所以上のruntime authorityや一API追加で多数の独立表現を手修正する再現例が出た時点でcatalog化を行う。

### P0-06 checkpointとevidenceの構造肥大化

* **判定：** 根拠付き対応不要（size単独のfailureはなく、authority境界は既に分離）。
* **調査結果：** `docs/current-checkpoint.json`は262,693 bytesで、最大groupは`activeRuntimeCheckpoint`の約205 KiBだった。Local measurementでは同じ文字列の100回parseが72.63 msで、単発parse／validationは開発gateの障害になっていない。Schemaはscenario、historical、memory、browser、engine、active runtime、next stopのtop-level groupに分かれ、stable architecture／contract、raw trace、private reportは別文書／ignored artifactに分離済みである。
* **変更内容：** File分割は行わず、schema 3の`scenarioMilestones`を独立groupとして追加した。`docs/current-state-manifest.md`へgroup authorityを追記した。
* **検証結果：** `npm run check:docs`と`node tools/test-current-checkpoint.mjs`がschema、exact root groups、cross-group identity、generated READMEを検証する。100回parse measurementでもunbounded parse costやresource failureは再現しなかった。
* **未対応理由：** 現在値を複数fileへ分けると`docs/current-checkpoint.json`を唯一のcurrent authorityとする規則を弱める。File sizeだけを理由に分割しない原則に従い、異なるowner／lifetime／failure domainまたは実測上のfailureが現れるまで維持する。

### P0-07 証跡データのサイズと生成コスト

* **判定：** 解決済み。
* **調査結果：** 通常の最新checkpointでNode reportが207,405,914 byte、Browser reportが約24 MBとなることを再現した。benchmarkだけはsummary化済みだったが、必須private-target gateはfull payloadを保持していた。
* **変更内容：** 通常gateをsummary evidenceへ変更した。Node／Chromeは全callをguest-visible fieldへ正規化し、nested callbackの予約sequence順に`sha256-canonical-json-v1`を独立計算する。詳細call、writeback、host-delta列は明示的なfull診断に限定し、通常のNode／Browser reportへ各2 MiBの上限を設けた。
* **検証結果：** `npm run check:runtime:bgi-browser:prepared`で17,841 call、11,807,189 instruction、次stopを維持し、双方のdigest `9e8cd6e57ed3073c4e6325ab629f4159e85706717f02bfb9b064cad3441ef7c3`が一致した。Node reportは50,007 byte、Browser reportは193,040 byteだった。
* **未対応理由：** なし。raw/full evidenceは障害解析用として意図的に残すが、通常gateからは外した。

### P0-08 証跡契約の内部実装への過剰結合

* **判定：** 解決済み。
* **調査結果：** 従来のNode／Chrome比較はfull call objectを直接deep-compareし、`implementationId`を含む内部field変更にも影響される構造だった。
* **変更内容：** acceptance digestをmodule／symbol、arguments、return、LastError、guest write、ABI stack relation、logical resource stateに限定した。handler IDは対象外とし、内部構造検査は既存のRust／Worker structural contract testへ残した。
* **検証結果：** `node tools/test-semantic-call-digest.mjs`でproperty順、module名case、内部handler名、nested completion順がdigestを変えず、duplicate sequenceが拒否されることを確認した。実Node／Chrome runでも同一digestとなった。
* **未対応理由：** なし。

### P0-09 永続化の製品経路

* **判定：** 根拠付き対応不要（未到達product milestoneとして正しく管理）。
* **調査結果：** Reviewの現状認識どおり、primaryのwrite／flush／close／SQLite commit／browser restart／loadは未成立である。Controlled fixtureのpersistent byteはsame-session browser storage feasibilityだけで、product saveの証拠には使われていない。Roadmap row 3、`Q-SAVE`、`REQ-FS`／`REQ-REG`、restart-safe acceptanceが、一件のsaveをwriteからrestart後loadまで通すvertical gateを既に定義している。
* **変更内容：** Scenario manifestへ`save-browser-restart-load: not-reached`を追加し、controlled same-session byteとの境界と、writable overlay／restart／loadが未実装であることをREADMEへ生成した。
* **検証結果：** `tools/test-current-checkpoint.mjs`と`npm run check:docs`がnon-claimをcurrent authorityから生成することを検証する。Private controlled fixtureはpersistent byteだけを検査し、product save合格へ昇格させない。
* **未対応理由：** Product persistenceはsource immutability、Rust logical write-set、server committed generation、kill recoveryをまたぐsubsystem gatewayであり、API単体patchとしてレビュー中に追加できない。Roadmapのexit gateに従ってprimary corpusが到達した時点でvertical sliceとして実装する。

### P0-10 Thread／Wait／Timeout／Exceptionの一般経路

* **判定：** 根拠付き対応不要（foundationと未到達一般semanticsを既に分離）。
* **調査結果：** Runtime WorkerはthreadごとのCPU contextを保存／復元し、owner threadがhost request待機中でも別guest threadを実行できる。Controlled fixtureはmulti-context scheduling、nested callback、wait、cancelを検査する。一方、general CreateThread、wait-any／all、timeout、SEH／VEH／APCは未完成で、roadmap row 2と`Q-THREADS`がprimary traceおよびcreate／block／wake／timeout／cancel／exception fixtureをexit evidenceに指定している。
* **変更内容：** 新しいscheduler semanticsは追加していない。Scenario milestoneのevidence boundaryとP0-01記録で、controlled foundationをprimary一般経路の完成と誤認しないようにした。
* **検証結果：** Private controlled browser fixtureのthread／callback／cancel／watchdog assertionsと既存Rust scheduler／wait testsがfoundationを検査する。Current checkpointはgeneral thread／exceptionをscenario completionとしてclaimしない。
* **未対応理由：** Required delivery semanticsはprimary corpusで未観測であり、推測による一般Windows threading／SEH実装はactive scopeを超える。`Q-THREADS`のoracleとfixtureが揃ったgatewayでbounded behaviorを確定する。

### P1-01 `vine-core`と`RuntimeKernel`への責務集中

* **判定：** 根拠付き対応不要（aggregate rootの責務違反は再現しない）。
* **調査結果：** `RuntimeKernel`は4,936行／232個のpublicまたはcrate-visible methodを持つが、18個のsubsystem owner fieldは全てprivateである。Process、address allocator、threads、scheduler、messages、display、input、USER32、GDI、timers、objects、COM、heaps、files、module records、synchronization、modules、registriesはそれぞれのmoduleがcollection、allocator、plan／commit、validationを所有する。Kernel内の追加stateはcross-subsystem mutation planまたはborrowed snapshotであり、owner collectionの第二authorityではない。
* **変更内容：** Kernel／crate分割は行っていない。P1-10のsource-response validationはownerである`runtime/files.rs`へ置き、P1-06／07のCPU execute stateはadapterへ置いたため、今回もaggregateへ新しいcollectionやallocatorを追加していない。
* **検証結果：** `cargo test --workspace --locked`とclippyに加え、source file、memory writeback、USER32、COM、schedulerのowner別testでplan／commit、stale、wrong-owner、failure atomicityを検査する。Code inspectionでkernel fieldを直接borrowする外部APIがないことを確認した。
* **未対応理由：** File sizeやmethod数だけは分割条件ではない。Execution、kernel、Win32、host I/Oを別crateへ移すと、現在privateなplanを公開contractへ昇格させ、cross-subsystem transactionを逆に広げる。異なるlifetime／failure domainを持つ新owner、または変更競合の実例が現れるまで現在のaggregate boundaryを維持する。

### P1-02 公開APIによる内部構造の露出

* **判定：** 根拠付き対応不要（外部stable APIとして公開されていない）。
* **調査結果：** `vine-core/src/lib.rs`には20個の`pub use` groupがあり、多数のrecord／ID／snapshotをintegration testsと`vine-runtime-wasm`へ公開している。一方、workspaceの`publish = false`が全crateへ継承され、repository外consumer、semver互換性、third-party releaseはactive scopeにない。Mutable subsystem owner fieldとmutation planの大半は`pub(super)`／`pub(crate)`で、Wasm bridgeは`RuntimeKernel`のcommand／queryを介している。
* **変更内容：** Re-export削減は行っていない。新規fuzz targetは既存の意図的なbinary ingress APIである`CpuWritebackBatch::decode_binary`と`RuntimeSourceReadHostResponse::decode_binary`だけを利用し、内部collectionを公開していない。
* **検証結果：** `cargo test --workspace --locked`と`cargo clippy --workspace --all-targets --all-features -- -D warnings`でworkspace consumerを検査した。`Cargo.toml`と`crates/vine-core/Cargo.toml`のpublish継承、kernel private field、Wasm consumerをcode inspectionした。
* **未対応理由：** Integration testから型を隠すためだけの大量なvisibility変更はbehaviorを改善せず、fixtureをcrate内部へ密結合させる。Multi-crateまたは第三者向けstable facadeをactive scopeへ戻す場合に、versioned view／opaque ID単位で別checkpointとして縮小する。

### P1-03 実行・検証ハーネスの巨大化

* **判定：** 根拠付き対応不要（責務混在による再現failureはなく、既にsession基盤とscenario選択を分離）。
* **調査結果：** Windows trace agentは13,526行、Browser runnerは1,594行、Wasm bridgeは2,873行、Workerは2,365行だった。一方、current BGI CLIは29行で、profile／checkpoint selectionは188行のmanifest module、evidence classification／digest／benchmark policy／browser protocol／engine adapterは個別moduleに分離済みである。Trace agentは一つのFrida injection unitとしてobserverを共有し、raw event authorityを別processへ複製していない。
* **変更内容：** File分割は行っていない。P1-11では実行harness自体を複製せず、同じcanonical scriptsをCI jobのfailure domainごとに分離した。Fixture／browser buildはartifact producerを一つにし、consumer jobが同じbytesを使う構造にした。
* **検証結果：** `npm run check:bgi-command-contract`、`check:runtime-worker-state-contract`、`check:ci-contract`、`node --check`対象53 filesでcommand selection、session state、workflow参照、syntaxを検査した。
* **未対応理由：** Frida scriptやscenario runnerを行数で分割すると、injected global call／thread parent orderingやprocess cleanupを跨ぐ新boundaryが必要になる。実際に独立lifetimeを持つservice、再利用される複数scenario、または変更競合が確認された単位だけを分離する。

### P1-04 Browser Worker session classの肥大化

* **判定：** 根拠付き対応不要（session lifetimeの不整合やleakは再現しない）。
* **調査結果：** `BrowserRuntimeSession`はengine loop、thread contexts、pending host requests、source fetch controller、watchdog、evidenceを一つのsession lifetimeで所有する。Worker globalは`activeSession`一件だけで、二重startを拒否し、終了時にidentity一致を確認してclearする。各startは新しい`RuntimeBridge.instantiate`とengine instanceを作り、cancel／fault／watchdogはpending request、fetch、audio、browser resourceを同じterminal transitionでteardownする。
* **変更内容：** Class抽出は行っていない。P1-07でexecute denialをengine faultからsession structured stopへ変換し、P1-08でreport digest／writebackをbounded化したが、Windows stateやthread identityをhelperへ移していない。
* **検証結果：** `npm run check:runtime-worker-state-contract`、`npm run typecheck:runtime:browser-worker`、controlled Chrome fixtureのnormal／cancel／watchdog cleanupが、one-active-session、terminal idempotence、pending request retirement、fresh runを検査する。
* **未対応理由：** Execution controller、thread manager、watchdog、collectorは現在すべて同じsession terminal transitionに参加する。単なるclass分割はstateを相互参照するpublic mutable surfaceを増やす。独立再利用または異なるlifetimeが成立したcomponentから抽出する。

### P1-05 Wasm bridgeのsession状態とtarget処理

* **判定：** 根拠付き対応不要（multi-sessionまたはmulti-title authority競合はactive scopeで再現しない）。
* **調査結果：** Wasmの`thread_local RefCell<Bridge>`は一つだが、各Browser sessionは別Wasm instanceを生成し、Workerは同時sessionを一件に制限する。各prepareはinput／outputをclearし、`runtime`を新しい`BrowserRuntime`へ置換する。`BrowserRuntimeTarget` enumはcontrolled fixture、source-read fixture、fingerprint済みBGIを明示的に区別し、BGI hashは任意session stateではなくactive compatibility guardである。
* **変更内容：** Generic target-profile frameworkは追加していない。P1-06でbridgeが参照するengine metadataをadapter ABIへ限定し、target固有値をexecution engine internalsから分離した。
* **検証結果：** Controlled fixture、controlled source read、private BGIの三prepare path、duplicate Worker start rejection、session cleanupをfixture E2Eとprivate target contractが検査する。Code inspectionでTypeScript側にguest handle／COM identity／thread stateの第二authorityがないことを確認した。
* **未対応理由：** Active completion targetはfingerprint済み一タイトルで、multi-title一般化はdeferredである。未到達targetのconfiguration frameworkを先に追加すると、identity guardを弱めるか未使用schemaを固定する。第二の実到達targetまたは同一targetのversioned variantが必要になった時点でtarget profileを入力化する。

### P1-06 v86内部実装への依存

* **判定：** 解決済み。
* **調査結果：** Adapter consumerがlinear-memory byte offset `664`（memory size）と`812`（instruction counter）を読み、execute checkは上位側metadataとprivate layoutへ依存していた。v86更新でlayoutが変わってもABI mismatchとして検出できない状態を再現した。
* **変更内容：** Adapter ABI version `1`、memory byte length、instruction counter、execute bitmap ingressを正式exportした。全production／fixture consumerを`ExecutionEngine`／adapter profile経由へ変更し、private offsetを除去した。Raw upstream paging smokeだけはadapterを有効化しないhistorical baselineとして明示的に保持した。
* **検証結果：** `npm run check:execution-engine-contract:prepared`がsource revision、adapter ABI、CPU context v1／640 bytes、22 imports／814 exports、memory／counter exportをactual Wasmで検査する。Windows artifactは1,857,800 bytes／`99a0948c...954f`、Linux artifactは2,116,017 bytes／`21b0fb4f...20c4`で、両方のsemantic adapter probeがgreenとなった。Patch reverse-checkも成功した。
* **未対応理由：** なし。Raw upstream smokeのprivate state readはv86そのものの非adapter baselineを測るための意図的な隔離で、production contractには到達しない。

### P1-07 x86 instruction fetch境界契約

* **判定：** 解決済み。
* **調査結果：** 旧gateはEIPから固定15 bytes全体を先に検査し、page末尾の短い命令でも次pageがNXなら拒否した。一方、実decode途中で別pageへ跨ぐ場合のfault byte、register／EIP failure atomicity、slice間protection変更のcache behaviorが固定されていなかった。
* **変更内容：** Adapter-owned 4 GiB execute bitmapを追加し、opcode、prefix、ModRM、immediate等、decoderが実際にconsumeするbyteごとにexecute permissionを検査するようv86 patchを変更した。Denied fetchはreason `3`、PFEC相当`0x15`、CR2に最初のdenied byteを返す。Validated executable pageはslice-local cacheし、budget callごとに破棄する。
* **検証結果：** Actual adapter testがpage末尾one-byte instructionの成功、次page先頭NX opcodeの拒否、cross-page `mov eax, imm32`のEIP／EAX不変、両page RX時の成功、slice間RX→NX invalidationを検査した。Workerは同じstopを`execute-protection`として報告する。Entry、memory、window、Browser Worker fixture regressionsもgreenである。
* **未対応理由：** なし。General Windows exception deliveryは別scopeであり、この項目はCPU fault observationまでを閉じる。

### P1-08 benchmarkの代表性

* **判定：** 解決済み（現在到達しているprimary workloadの代表性を修正）。
* **調査結果：** 旧benchmark fixtureは17 calls時点のcheckpointを固定しており、current 83-contract／17,841-call runtimeへ再実行できなかった。単純起動反復だけでなく、current User32 window-position／activation pathまで進む継続sessionが必要だった。
* **変更内容：** `bgi-browser-runtime-user32-window-v1` fixtureを追加し、11,807,189 instructions、17,841 calls、`USER32.dll!MonitorFromWindow` stopをcurrent capacity workloadにした。Execute lookupをslice-local page cache化し、dirty discoveryをdirty-only scratch、writebackをborrowed engine viewからWasm inputへのdirect copy、semantic digestをstreaming SHA-256へ変更した。既存17 thresholdsは緩和していない。Full public benchmark再検証では、exact instruction counterにより8命令setupがowned thunk直前でbudget stopになることを検出したため、timed region外のbounded setupを16命令へ明示し、failure diagnosticへreason／error／count／EIPを追加した。同じrunがCPU stack reconciliationの外側transaction内でimmediate API completionを行うとnested transaction panicになる回帰も検出したため、completion guest writesを既存journalへ参加させた。明示的nested beginは禁止のままである。
* **検証結果：** 3分gateは32 sessions／failure 0、session P95 6,035.420 ms、slice P99 4.815 ms、Worker heap max 29,115,120 bytes、capacity max 65,441,792 bytes、backing max 19,737,862 bytes、core linear 10,092,544 bytes、engine linear 14,024,704 bytesで全thresholdを通過した。Semantic digestはNode／Chromeとも`9e8cd6e5...f7c3`で不変だった。修正後のpublic browser gateもinteger 116,969,588 ips、production API 7,108 roundtrips/s、steady memory 13,344,304 bytes、background ratio 1.018、failure 0で`go`になり、private 17-call replayは238 instructionsで次の`EncodePointer` stopへ到達した。Rust regressionは外側transaction内の8-byte API outputについてcommit反映とdrop rollbackの両方を検査する。
* **未対応理由：** Graphics frame、audio stream、asset streaming、input、saveはprimaryで未到達のため、このbenchmarkへsynthetic workを混ぜてproduct代表と主張しない。各subsystem gateway到達時に同じversioned suiteへscene-specific workloadを追加する。

### P1-09 テストコードの保守コスト

* **判定：** 根拠付き対応不要（行数は大きいが、指摘されたsetup／期待値重複は既に局所化）。
* **調査結果：** `api_dispatcher.rs`は8,897行、`user32_dispatcher.rs`は6,128行だった。ただしUSER32には7個のtyped fixture builder、両fileには合計58個のtable-driven loopがあり、pointer validation、callback、lifecycle、failure atomicity、state transitionを一つのfixture内でpositive／negative pairとして検査する。Property testsはguest memory専用file、owner model testsはsubsystem別fileに分離済みである。
* **変更内容：** Test fileの機械的分割は行っていない。新しいsource response casesはowner unit testの一つのtableへ追加し、新しいexecute casesは共通actual-adapter contractへ追加した。Transaction composition regressionも既存dispatcher fixtureを再利用し、API名ごとの新fixture fileは作っていない。
* **検証結果：** `cargo test --workspace --locked`で全integration／unit tests、clippyでall targetsを検査する。Source responseの5 malformed casesとvalid success／errorは同じbuilderを共有し、execute boundary variantsも同じengine setup、outer transaction commit／rollbackも同じdispatcher setupを共有する。
* **未対応理由：** 行数だけでpositive／negative transaction pairを別fileへ分けるとsetupとbefore-image assertionが重複する。具体的な変更競合、同じsetupの三重複、または異なるowner／failure domainが現れた部分から分割する。

### P1-10 fuzz対象の限定

* **判定：** 解決済み（到達済み高リスクbinary ingressを追加）。
* **調査結果：** Coverage-guided targetはPE planner一件だけだった。さらに`RuntimeSourceReadHostResponse::decode_binary`は32-byte controlを解釈後、request上限やresult shapeを検査する後段より先に任意長host payloadを`Vec`へcopyでき、host responseによるunbounded allocation経路を再現した。
* **変更内容：** Source response decoderがcopy前にnonzero／最大1 MiB request、transferred <= requested、successのexact payload length、errorのzero transfer／empty payloadを検査するよう修正した。`cpu_writeback_decode`と`source_read_response_decode` fuzz targetを追加し、`fuzz/Cargo.lock`を同期した。CIは各target 15秒、weekly workflowは各target 30分の独立campaignとした。
* **検証結果：** Rust unit testはvalid success／errorとzero、oversize、over-transfer、length mismatch、error payloadを検査し、source-file integration regressionもgreenだった。WindowsではMSYS2 UCRT `clang++ -fms-extensions`による全fuzz bin typecheckを通した。Windows GNUはASan非対応なのでcampaignはpinned Linux nightly `2026-07-10`／cargo-fuzz `0.13.2`で実行する。
* **未対応理由：** 未到達graphics／audio decoderやwritable VFS schemaはまだ入力contractが存在しないためtargetを先行作成しない。到達済みpath／Windows-932はmodel／property negative testsを保持し、coverage gapまたはcrashが示された時に専用targetへ昇格する。

### P1-11 CIの単一長時間job

* **判定：** 解決済み。
* **調査結果：** 旧workflowはUbuntuの`public-checks`一件で`check:fast`、fixture E2E、Chrome、benchmarkを直列実行し、Rust failureでもbrowser setupと同じjobを共有していた。Failure owner、required check選択、並列化が曖昧だった。
* **変更内容：** `check:fast`を`check:static`、`check:toolchain-engine`、`check:rust`のlocal aggregateへ整理し、fixture E2Eをcore／browser prepared groupへ分けた。CIはstatic、Rust、fixture producer、core E2E、browser acceptance、performance、stable observation、Windows oracle、fuzz smokeの9 jobsへ分割した。PE／adapterとbrowser buildは一度だけ生成し、downstreamへartifactとして渡す。最初のremote runでRust jobだけがproducerより先に開始し、ignored `.work/toolchain-spike/abi-smoke.exe`を取得できないことを検出したため、Rust jobにも明示的な`fixture-build` dependencyとartifact downloadを追加した。同じrunでNode 22.23がbenchmark serverの固定port `4190`をunsafe portとして拒否したため、OS割当loopback port `0`へ変更した。
* **検証結果：** `check:ci-contract`がworkflowをYAML parserで読み、exact job set、Rust／core／browserのfixture dependency、npm script、Chrome role、benchmarkのephemeral port、fuzz target／budgetを検査する。`actionlint 1.7.12`でも両workflowを検査する。Local canonical `check:fast`／`check:fixture-e2e`は同じbehaviorを保持し、fresh CIでもgenerated fixtureなしにRust testを開始せず、performance jobはNode／Fetchのunsafe-port listに依存しない契約にした。
* **未対応理由：** Repository settings上で各jobをrequired checkに選ぶ操作はGit管理外である。Workflow側では独立check名とfailure domainを提供済みで、branch protectionを導入する場合に選択できる。

### P1-12 Chrome stable依存

* **判定：** 解決済み。
* **調査結果：** 旧browser E2Eとbenchmarkは`chrome-version: stable`を共用し、同じsource commitでも自動更新によりacceptance環境が変わった。Chrome for Testing `151.0.7922.34`のLinux artifactが取得可能であることを確認した。
* **変更内容：** Required browser acceptanceとperformanceをexact `151.0.7922.34`へ固定した。Moving stableは同じbuilt browser artifactを使う別の`continue-on-error` observation jobに分離した。
* **検証結果：** HTTPS 200でexact artifactの存在を確認し、YAML contractとactionlintが固定version／stable分離／dependencyを検査した。Browser reportは実行時のactual browser versionを引き続き記録する。
* **未対応理由：** Stable observationをrequired acceptanceにはしない。固定revision更新はbrowser compatibility確認後の意図的なcheckpointとして、contract testと文書を同時更新する。

### P1-13 Windows native継続検証

* **判定：** 解決済み（redistributable oracle band）。
* **調査結果：** `run-toolchain-spike.mjs`はWindows上でABI、browser vertical、source read、DLL lifecycle、window pixel、FS、process parameters、TLS、FLSの9 PE32 fixtureをnative self-oracleとして実行する。remote run `29706968785`／`29707055841`では、Windows runnerのhost display modeがVineの固定1920x1080／60 Hzと異なるため、`browser-vertical.exe`がexit code 6になることを再現した。これはguestの論理display profileとnative host topologyを同一authorityとして扱ったfixture契約の欠陥だった。
* **変更内容：** `browser-vertical`のnative D3D9検査を、非zero・boundedな`D3DDISPLAYMODE` shapeへ変更し、Vine adapter identityが返るguest/browser経路では従来の1920x1080／60 Hz／format 22を厳密に要求するよう分離した。`windows-2022` jobのllvm-mingw 20260616 UCRT archive（SHA-256 `b9b68a4d...ca35`）とrunner LLVMの`--print-targets`検査／`VINE_WASM_CLANG`分離は維持した。fixture hash、Wasm bridge fingerprint、runtime-core identity、toolchain／CI文書を同じcheckpointで同期した。
* **検証結果：** local Windows `npm run check:toolchain`は9 native exitsとartifact inspectionを通過し、`npm run check:runtime:browser-worker:prepared`は1,945 instructions／233 yields、pixel／audio／storage／source-read、cancel／watchdog cleanを再現した。修正後のremote run `29707529941`（HEAD `9e374ce`）でもWindows native oracleを含む9 jobs（static、Rust、fixture、browser、performance、stable、fuzz）がすべてsuccessとなった。旧remote runsのfailureはこの修正前の証跡として残し、再発していないことを確認した。
* **未対応理由：** Primary BGI native traceは配布不能corpusとinteractive sessionを要するためpublic CIへ入れない。Public jobはWindows loader／ABIとhost-independent fixture contractの継続性を担当し、title oracleはcommit-bound local gateとfocused traceに残す。

### P1-14 文書とcheckpoint情報の維持負荷

* **判定：** 根拠付き対応不要（current authority重複は既に防止され、historical evidenceを削除する根拠がない）。
* **調査結果：** Markdownは136 filesで、62 `bgi-*`と29 `v86-*`がfrozen historical evidenceに分類される。Current値を参照できる文書は7 filesに限定され、API count、call count、generation、report identity、next stop、canonical commandの複製をvalidatorが拒否する。README Current Statusは`current-checkpoint.json`から生成される。
* **変更内容：** Historical evidenceの削除／一括統合や新registryは追加していない。今回の新しいstable contractはVFS、CI／testing、Windows developmentの既存owner文書へ追記し、API occurrenceごとの新文書を作らなかった。
* **検証結果：** `npm run check:docs`が136 files、233 IDs、link／table／fence／current-value authority、README生成を検査しgreenだった。Current manifest／README drift testもgreenである。
* **未対応理由：** 91個のfrozen文書はraw traceではなく過去decision／oracle／non-claimで、active queueとして更新されない。削除はevidence provenanceを失い、単一registry生成は独立oracleを自己検証させる。今後も定型statusはmanifestから生成し、新semantics／architecture／oracleがないAPI単位文書は追加しない。

## 最終再検証

* **調査結果：** 上記P0-01〜P0-10、P1-01〜P1-14をP0→P1順に再確認した。各項目は解決済み、またはactive scope・到達証拠・設計不変条件に基づく対応不要として判定されており、未判定項目は残っていない。未到達のfirst present、product audio、実入力、save／restart／load、一般thread／exceptionは、fixtureの成立性と混同せず、current manifestの`not-reached`／`fixture-only`境界を維持した。
* **変更内容：** `d5ce1b6`でCI fixture依存、PE／Wasm toolchain分離、benchmarkのephemeral port、production setup budget、transaction composition回帰を修正した。今回のD3D9 native／guest境界修正に伴い、browser fixtureを3,584 bytes／SHA-256 `7e584ea6a241b09715ff85d855755aca8ce5a01a3f073edce0fcd65c24cda7fd`へ更新し、runtime-core Wasmの実測identity（3,352,778 bytes、SHA-256 `12c75144628bd0d7390bc5298e55cc2e6756f8f6b3732ce4ab35f1bd32fcd02d`）を`docs/current-checkpoint.json`とvalidatorへ同期した。private gateは最終implementation commitに対して再生成する。
* **検証結果：** `npm run check:docs`、`npm run check:static`、`npm run check:private-target-contract`、`npm run check:private-target-evidence`がgreen。`npm run check:private-target -- --allow-dirty vine.zip`はNode／Chromeとも17,841 calls、11,807,189 instructions、structured stop `USER32.dll!MonitorFromWindow`、cancel／watchdog clean、benchmark failure 0で終了した。生成evidenceはvalidatorのpayload digest、implementation commit／tree、artifact identity検査を通過しており、実行ごとに変わるtimestamp／report digestを手書きのcurrent authorityへ複製していない。`vine.zip`はreview用の無関係な未追跡pathとして許可リストにのみ残し、commit対象から除外した。
* **未対応理由：** 残る対応不要判定はレビュー上の未解決ではなく、AGENTS.mdでdeferredと定めた未到達scopeまたは独立oracleが必要なgatewayである。primary product milestoneへ昇格する条件は各項目の記録と`docs/roadmap.md`／`docs/risks-and-open-questions.md`に残しており、根拠なしの実装拡大は行っていない。
