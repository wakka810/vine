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
