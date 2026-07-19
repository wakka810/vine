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
