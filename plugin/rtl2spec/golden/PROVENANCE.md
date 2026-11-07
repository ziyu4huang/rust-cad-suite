# golden PROVENANCE(機械生成禁手改;--record 機械重寫元資料區+歷史區保留追加錄製行;語義輪手記入歷史區)

## 正式釘選集 49 檔×2 模式(§16.1.2;37 檔=§16.1.1 表,其餘=收尾弧 S3 擴檔,見 RTL2SPEC-FINISH-PLAN §11)

- 錄製命令: bash plugin/rtl2spec/run-golden-regression.sh --record
- 錄製時間: 2026-11-07T10:58:40.649Z
- bun: v26.3.0; wasm ABI: 0.1.0; wasm sha256: 66518257be803ebd70c62fd284c09f8b78b83425b8300244ef79e8da4b3d707d
- 錄製來源: 提取器真實運行輸出(禁手寫);re-record=語義事件,錄後必讀
  git diff plugin/rtl2spec/golden 逐案對帳——期望變了而提取器沒變=必須能解釋,
  否則停(§16.1.4;delta 分類=語義修復/缺陷修復/意外,寫進下方歷史區)
- 自檢: 每檔每模式進程內 3 跑 hash 全等(兩階段;任何檔不過=全輪拒錄 exit 3)

| golden | sha256 |
|---|---|
| yosys-simple/operators.rules.json | 0c8ce866a52c6a6a19f886d6c81ca47eda52aef7cf790b541533dc567514bd97 |
| yosys-simple/operators.gaps.json | 734bb157dfa2897718926b3dd736e819835df55c3c28a897cea043ed24764df2 |
| yosys-simple/memory.rules.json | 6f30f9760c09aca4baca0b2d3e67199849f285957ef032c447620bc45bb9c118 |
| yosys-simple/memory.gaps.json | b08fcdec117231090aeec91c2f5b0ec8ee232cdf2d8f3b2bf87a319835fadf23 |
| yosys-simple/constmuldivmod.rules.json | 71d6473d36e55a21335a3862f4bdece2bab68c2b3bae209f32ff72f948ed3377 |
| yosys-simple/constmuldivmod.gaps.json | 9d1e183c0254bda33a0ad36d5c4d2febc29e75ce7fdf4e11393ce5dddbfb5a90 |
| yosys-simple/sincos.rules.json | b7a2a07bb4c742b96dfa84aa39393c6f9726240ffd27c84c2d1442aa85f481f1 |
| yosys-simple/sincos.gaps.json | 36d6137f98dccbfd5a9a20d3d25550f146f770856a4ebe49432fd1b1706e259e |
| yosys-simple/partsel.rules.json | 2168735f5fc615c8c682ef17a7387229553152adfc9d339033df14a87c9ec2c7 |
| yosys-simple/partsel.gaps.json | 6fb56730c43a4f48f805d321a28921fc0a414c25a460820dd0cefa1f2ad69e57 |
| yosys-simple/fsm.rules.json | 077f09435067831dab05b131c46779e96fee4396237bc278bb501990a9c7d961 |
| yosys-simple/fsm.gaps.json | d7e5d3482903d09dc0c6adf428e9d153fe1354070475227f6fa802bb67ac28f4 |
| yosys-simple/dff_different_styles.rules.json | 3adc369cabaa402e3ad34cb15d88be271332e2d50d689d1e5bad5be94e6d7777 |
| yosys-simple/dff_different_styles.gaps.json | 8fda9e7b6395c177f35aa7d90bbf0cc1bf63f2a718b49605fbbc8e33b5bd67a1 |
| yosys-simple/generate.rules.json | c8806a515b8abac6548f89b835911e401216b2c6ed24b1143014867c891af683 |
| yosys-simple/generate.gaps.json | 3ccf4d75be1619a51d6ff0aa6c5c5d4fe5802b4e649f91ec8669eba3361a0645 |
| yosys-simple/process.rules.json | 9f476243fc7b9e5e160ce6d0d9f946ab1fc2cb3401825229f0f575ef256416a6 |
| yosys-simple/process.gaps.json | f91910127c91722192dd1b2350aac186c7ef533eb6fc6f1d49db34590a01cb70 |
| yosys-simple/fiedler-cooley.rules.json | 13a16fffff037f7927a27eb96c21d13fa010f50f46e46a6bdf3fff21fedc468f |
| yosys-simple/fiedler-cooley.gaps.json | 08af5e4d9d46be9c106f7167b87fe36275f2166aea65f9a96ea0ed1809f08936 |
| yosys-simple/multiplier.rules.json | 9f97877649473b9c71c55a39bbdf7749d79ecfdfc3afe37b1fc4f67e44874220 |
| yosys-simple/multiplier.gaps.json | 8eb46bd73faa62a039b7c80384e3d8a98c641d3817083d3fc19bd1d2e50e75d7 |
| yosys-simple/i2c_master_tests.rules.json | e6bef3c9cd1ee92abb9545b77cf2859417d03f525e018a89940c6301fab58c59 |
| yosys-simple/i2c_master_tests.gaps.json | 85d5e61021b3df785876169c0806a0c152abcffa57334b4003bbee757fd8139a |
| yosys-simple/undef_eqx_nex.rules.json | e14732823c6d1e9bdf50765c90efcea5e64dc58cec3f5b835fcf6f6c1b93706f |
| yosys-simple/undef_eqx_nex.gaps.json | 9f62a7827d806e6a1aefa73fdd3a6f70a329900784ce2a4d16f2c409d71c8374 |
| yosys-simple/always03.rules.json | 3f0ee477be20f7250cbb4bd7846ca766554e52ad8f0334137c608af2bf01db83 |
| yosys-simple/always03.gaps.json | 6466c1249247dde7e6dfc9f9e5f568b2595192b32a9d4b8ff064aaedf3167dd3 |
| yosys-simple/forloops.rules.json | 1be841c9af02eac26998653e1439b00467cec1f00b57144af35e2c4efaf2b527 |
| yosys-simple/forloops.gaps.json | 6b4006bed36380d6645757aab58da52937ae88e964d32efd1b8a997edddcd49f |
| yosys-simple/realexpr.rules.json | ff53656749b7a197bdcad3dd84efbea29650dd3a24aa4bea8037a4f644a6b784 |
| yosys-simple/realexpr.gaps.json | 2b44c982761b53808a2489d92d809317e1a488b694ec074494e982e98d93b3f2 |
| yosys-simple/subbytes.rules.json | 1a5aa2b5c150f4ee80352715f60eb96b2b4000eb3b3a0ad3f6b7435af416f7cc |
| yosys-simple/subbytes.gaps.json | 1d39b5c13ff43887691f420d0fbc9c8cd00cf0e69d413de7bf00d2648f578955 |
| yosys-simple/signedexpr.rules.json | 037be8371fd5cb8d4ffeaa1bb8ebd27c1115c6ac2d75af6da4813e265d06ca5b |
| yosys-simple/signedexpr.gaps.json | b84800f07fc27d277f4c635bdec1af74b55aad8f454dc041f489758a627413c0 |
| yosys-simple/task_func.rules.json | d0c4fb08e3991df36149af194d408428ca9e842fe182bccff2afcce50323ae7c |
| yosys-simple/task_func.gaps.json | 6785905ce1e65631e492bf1518036c853edd4010e3068623a09c41338986b4e4 |
| yosys-simple/vloghammer.rules.json | b7e1da9673ed27a57c813ed3e43ba74b04a024fa4253aec413f3a455e7baa939 |
| yosys-simple/vloghammer.gaps.json | 737cf860a074f06826560341d9bb529584793812f4cbd27900e2bac07c078ca4 |
| yosys-simple/wreduce.rules.json | 85651fa4ab9d658daeba5df34819403d761c05f3e080c552a3bd6d0828e24115 |
| yosys-simple/wreduce.gaps.json | ac3b24e041ca3b2c76dddc71bfaf3e22ffee03c9d864b33ad3f7c10fc7bb6f07 |
| yosys-simple/muxtree.rules.json | ada731555d1f15f45642a28b9cc14088ff834fe16bb358a41a9d568c4f382f5d |
| yosys-simple/muxtree.gaps.json | ebc31a75e87a487c8377e69303c2314f744d27b8a1f4f1b727114f6a57cc0297 |
| yosys-simple/dff_init.rules.json | 1f8c06475b3ec0f34dc753de977cf9afd2390675ad353d93b29a7d98cca4ff2e |
| yosys-simple/dff_init.gaps.json | 8712ec4f0651c5a5d7b69e1053995495932fab4ffc0ff5842d9b4ea5eceb6147 |
| yosys-simple/mem2reg.rules.json | 1d21038815b2e975623b518ff7910508a57b4f99fe29a67c707b194e04bf7413 |
| yosys-simple/mem2reg.gaps.json | 6966759645d72d6e96a8f1b47e99927a98cdb86c063644f3fd1f5796abd7fbeb |
| yosys-simple/macros.rules.json | e9715bb2d2164c7de4f67087d537fce5950f3fc1bc01c4d82f55447dee847cdb |
| yosys-simple/macros.gaps.json | 848b6a6ccd95fb1c4e355c73fe4e51b069ebf823619f5ff73826f29af058625b |
| yosys-simple/aes_kexp128.rules.json | 3b2d439953321b16d41b589a74729d9e8f1c608b84ee714dd4cf7156ef4dccd0 |
| yosys-simple/aes_kexp128.gaps.json | 1742e2599f2e5ecfd416f109b6c49f81d1552afb35139f2e8d2456947afc07f7 |
| yosys-simple/carryadd.rules.json | f4a6a2cdb507d16153faf1c8a6ad82953edf7b73de03d8fcafc08dcbb4eb435b |
| yosys-simple/carryadd.gaps.json | 39769828469b2e81c27ab78942faf77dfcc2c2198886336e80f24de17c60a205 |
| yosys-simple/loops.rules.json | 6ed9e6abca2e66f034a656af07ac8126b122daf5781f7b0f7b91b6823f652295 |
| yosys-simple/loops.gaps.json | 5b585f5e905aedfa744dd626c4d9eb73c8d36b7318042e62bb4b0aa6d13e604f |
| yosys-simple/attrib04_net_var.rules.json | ad085f8b4a83b95ea415f78f731a68bdd9380ae6353425ba28fd4b2f7fb4fe73 |
| yosys-simple/attrib04_net_var.gaps.json | a356b8e27e793ac1ac6c89ae5d490b262d83dacef2d0489a7ba2c97bf10e0d7c |
| yosys-simple/omsp_dbg_uart.rules.json | 86bda6a298daeee9632ad18e2ea3d7aa888a566e818da833f76f0e1b4c5f3e5e |
| yosys-simple/omsp_dbg_uart.gaps.json | c80722962aa96eb9d882aa3bc2ed96f6db7006e85fbc2dca1f494ac9c26d71da |
| yosys-simple/mem_arst.rules.json | 431453d53303e7ff5a0374add1d4f292511c3edeae1516f073d9821c374824d6 |
| yosys-simple/mem_arst.gaps.json | 2fcb60364eb61315d1f8f3db45555e746bae240ebf42f379d0c4f7ef9be50234 |
| yosys-simple/forgen02.rules.json | f3d16d5eb336b6f7df40a7dc2e761859e5db0552786eb57f21e9939c272cf051 |
| yosys-simple/forgen02.gaps.json | 8dab481d0ebe58893e83fd2cdb97a48d489c82bc880afce35119c98c9f82bb91 |
| corpus/e2e/59_i2c_master/i2c_master.rules.json | ea6b4267bde15191254c9f5c7bdecf1c294aff9da390134f84e4fa6185ee1362 |
| corpus/e2e/59_i2c_master/i2c_master.gaps.json | d414618aa6cd7e3823183ed40a3736ae66ff3d2500b7ec8667d9c01295ab03fc |
| crates/spec-i2c/rtl/i2c_master.rules.json | cca8fbf136bafed3bc002bac9b000e3aeea8d983737e9f924cec337f91ba1b2a |
| crates/spec-i2c/rtl/i2c_master.gaps.json | a3b6bc98bf4200a54f11e2922543b5f98a05265ea0ebab5372496fe7d2ba9036 |
| corpus/e2e/58_xls_acc_pipe/acc_pipe.rules.json | 00d931c10c8dcf8e8011d715a61a6060fefea94b904f496d2a14e08a5d6a6610 |
| corpus/e2e/58_xls_acc_pipe/acc_pipe.gaps.json | f5f75fb023ec811a1f5071a45e4799896f5c8c1fab3d7921dfab71f9a5923c53 |
| corpus/e2e/57_xls_add3/add3.rules.json | c68e397c7330603d11cdfc64bd4c644012ee07edc49d0e755d21c001fac0060d |
| corpus/e2e/57_xls_add3/add3.gaps.json | a42dcae9a3b628b50ee3ec83cdd4cfbab1c48281fc8b4afef7b86e2f483d233c |
| corpus/fulladd/fulladd.rules.json | 293841f33181fc38dc171786a3a78a7cc8c5d5fb6ffa91fd659e7310c916241f |
| corpus/fulladd/fulladd.gaps.json | 5cc22e7593c7338ebebea6ca91b912c7f883323d52f4faac950acec898a20f84 |
| yosys-simple/always01.rules.json | 2e3acee2f821d5fea8814c9011807013e1f9c250cff3d7d576f8da3d97578733 |
| yosys-simple/always01.gaps.json | 9a3acb227ef1f8f0fe562d9ba78b4ea06cca769660366ef7ca8d42dab5842b1a |
| yosys-simple/always02.rules.json | 8eff7f9b529d51770d0648965bfd1bb8b3116cf436ad18ddc6309c39a20f4b9c |
| yosys-simple/always02.gaps.json | 0658649cf973e78c7e9f7af7fca1fd79de6d8f75e439e5e98fe1db67173eaae6 |
| yosys-simple/attrib01_module.rules.json | ed1083e9bb06a8b67949a8be732212079c86fca7523f364b438f7103ae2ba214 |
| yosys-simple/attrib01_module.gaps.json | f75d1b898050a56013b0a1839ad0f8d31ed15a75ee62347d0ac3ece520911959 |
| yosys-simple/attrib02_port_decl.rules.json | 37a8cbe030064e5633a1481b86b6ccfae7c46cb08bded103d6042ed5e938b3d1 |
| yosys-simple/attrib02_port_decl.gaps.json | 2e5a5b3cb432a8d17c6020b599ca118a0f09f5c53fe1a2b482944ad4aa59c021 |
| yosys-simple/attrib06_operator_suffix.rules.json | e06cf5fa987db54bdcb62d9865ed7e6806729f9401883527649812c65ce5be54 |
| yosys-simple/attrib06_operator_suffix.gaps.json | a692b7e92ed0928535b9a2c71a829f56b69537a1f8b1d5776f4fad1a2f74c16b |
| yosys-simple/attrib08_mod_inst.rules.json | 5fb149889b09fd4edaa3b9bfcc7c5f8946b8d7e532188fce8d854d43447dffcd |
| yosys-simple/attrib08_mod_inst.gaps.json | 52e5497f99a108d2f1dd1a4538e1f32c1b2b07164160e4b7baa503cca24a7ea0 |
| yosys-simple/attrib09_case.rules.json | 05888a01ebf5d774def967ed24620cc094c37ff9a0bba854005ac90cf55d0671 |
| yosys-simple/attrib09_case.gaps.json | a221935a574ebf7d2203640919c8017c330a0851f0cad4bbf918a32f65d31a8e |
| yosys-simple/graphtest.rules.json | a9841eb4e36fef19ab64d9c51061595a6941ae94ccf90397f0b090404a77000f |
| yosys-simple/graphtest.gaps.json | 1109fa9dcf029ca293fa0b56e0f4e2ff92217aead78e13523ee5e6d0208d34aa |
| yosys-simple/loop_prefix_case.rules.json | a34bdc4f008d4f2c6dc07a337bceadd333540faa41636bee6034668394ad9a9d |
| yosys-simple/loop_prefix_case.gaps.json | 393534ded64381650c1046fbd646c2493aaae515f8c20b1eae152dd666cd3779 |
| yosys-simple/retime.rules.json | 298de3a4c1e048afcc389b73de6499aa3edd2fede9b6a240126ea1e6265d2258 |
| yosys-simple/retime.gaps.json | adcd6ab4a6ae706b2225271e9846089cc2de5e2b499aaa4395ccd33478cc6d46 |
| yosys-simple/scopes.rules.json | 6854d494cdb4fce23659c0e532f45d45fb0c277b6c91903ac5114eff45e554ad |
| yosys-simple/scopes.gaps.json | eb6de22dd2eaa16c3101088a7095dc258ab227648237f9f420e8d109d4d29062 |
| yosys-simple/usb_phy_tests.rules.json | b507b78d21e8e902bde2d8d84caca1ec01705582038ad9a794bb95b8a0d15372 |
| yosys-simple/usb_phy_tests.gaps.json | 6488e45431dc418ced1f90b8042a86dbb187b56283171e2e9181a3dd142fd3a1 |

## demo 樣例(不屬本表)

- golden/plugin-fixtures/ 域=§17 G8 實證 demo(record-golden.mjs 錄製),
  歷史=golden/plugin-fixtures/PROVENANCE.md;run-plugin-check ④d 對其 byte-diff。

## re-record 歷史(語義事件日志;語義輪手記逐行)

- (首錄 2026-11-04,S9;F1a/F1b+F2 確定性修復後基線,§16.6 S9①對帳=全部可解釋首錄)
- (手記 S0 演練 2026-11-05;M7 修復驗收形:本行=人工手記形 fixture,驗證 --record 歷史區解析-保留-追加不覆蓋人工行)
- (re-record 2026-11-05T00:56:13.947Z; wasm sha256 4b143d3a…)
- (re-record 2026-11-05T01:35:04.642Z; wasm sha256 3d021e7c…)
- (re-record 2026-11-05T02:42:45.155Z; wasm sha256 71efe1c3…)
- (re-record 2026-11-05T03:08:15.383Z; wasm sha256 91b2922c…)
- (re-record 2026-11-05T03:16:57.980Z; wasm sha256 a942c640…)
- (re-record 2026-11-05T03:48:52.879Z; wasm sha256 23af31de…)
- (re-record 2026-11-06T19:10:09.291Z; wasm sha256 f689132b…)
- (re-record 2026-11-06T20:12:54.046Z; wasm sha256 9e9db744…)
- (re-record 2026-11-07T10:58:40.649Z; wasm sha256 66518257…)
