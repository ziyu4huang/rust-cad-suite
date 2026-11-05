# golden PROVENANCE(機械生成禁手改;run-golden-regression.sh --record 全量重寫本檔)

## 正式釘選集 37 檔×2 模式(§16.1.2)

- 錄製命令: bash plugin/rtl2spec/run-golden-regression.sh --record
- 錄製時間: 2026-11-03T21:56:09.307Z
- bun: v26.3.0; wasm ABI: 0.1.0; wasm sha256: 4b143d3afd22c41f85a0c2cebc23edd0c0176e3eb0f2ae88849fc4c8d855f3cd
- 錄製來源: 提取器真實運行輸出(禁手寫);re-record=語義事件,錄後必讀
  git diff plugin/rtl2spec/golden 逐案對帳——期望變了而提取器沒變=必須能解釋,
  否則停(§16.1.4;delta 分類=語義修復/缺陷修復/意外,寫進下方歷史區)
- 自檢: 每檔每模式進程內 3 跑 hash 全等(兩階段;任何檔不過=全輪拒錄 exit 3)

| golden | sha256 |
|---|---|
| yosys-simple/operators.rules.json | 5fe556458fb8ba556d30ca457f539904b8f5b9a19bc3946d619268066e44db5f |
| yosys-simple/operators.gaps.json | 734bb157dfa2897718926b3dd736e819835df55c3c28a897cea043ed24764df2 |
| yosys-simple/memory.rules.json | 8cc9d82578f04642dc2512955a2d1ad3fec206ca1bb39fb6cdb29cca5d0e1ced |
| yosys-simple/memory.gaps.json | b08fcdec117231090aeec91c2f5b0ec8ee232cdf2d8f3b2bf87a319835fadf23 |
| yosys-simple/constmuldivmod.rules.json | d6b9d1b1d9d7c02b6653e18753d21c5bb7a3c1cca44d626488f1320f718c0c55 |
| yosys-simple/constmuldivmod.gaps.json | 9d1e183c0254bda33a0ad36d5c4d2febc29e75ce7fdf4e11393ce5dddbfb5a90 |
| yosys-simple/sincos.rules.json | f742085d781585353d5b0fbcaf51a12c2a775f623bbdf080e98f8fa08acd6a85 |
| yosys-simple/sincos.gaps.json | 36d6137f98dccbfd5a9a20d3d25550f146f770856a4ebe49432fd1b1706e259e |
| yosys-simple/partsel.rules.json | d4c999607cbeb8ae25d2b6c1a0eb165d481b3459d48d2a865271c20af7d14bce |
| yosys-simple/partsel.gaps.json | 2b238738e95eb66ceab2488a6967f7574f528b03a5529815e9848aadcaf19494 |
| yosys-simple/fsm.rules.json | dd5f406d1872b404c7a22f91bfc8fea8eb74a6e37093dc153fd35ab2cd57f244 |
| yosys-simple/fsm.gaps.json | d7e5d3482903d09dc0c6adf428e9d153fe1354070475227f6fa802bb67ac28f4 |
| yosys-simple/dff_different_styles.rules.json | 0c94c690d2aaaa6b8a7e12ef86549225f4a06e34c6b600062aefb7419a943ef7 |
| yosys-simple/dff_different_styles.gaps.json | 8fda9e7b6395c177f35aa7d90bbf0cc1bf63f2a718b49605fbbc8e33b5bd67a1 |
| yosys-simple/generate.rules.json | a4cadfe10e8f8a7b67584fa5f3be60b6f0ad245b09b369d8d1c56b410c011d24 |
| yosys-simple/generate.gaps.json | 3ccf4d75be1619a51d6ff0aa6c5c5d4fe5802b4e649f91ec8669eba3361a0645 |
| yosys-simple/process.rules.json | f112c4594f66a02d69b45a77fc2d238706c07c5d07c743f79004cee35ada8530 |
| yosys-simple/process.gaps.json | f91910127c91722192dd1b2350aac186c7ef533eb6fc6f1d49db34590a01cb70 |
| yosys-simple/fiedler-cooley.rules.json | 05bfbfe5bbdc78d4b2dfd1f4fee4f193a0c79acab23adba70e5d30f683f9922a |
| yosys-simple/fiedler-cooley.gaps.json | 08af5e4d9d46be9c106f7167b87fe36275f2166aea65f9a96ea0ed1809f08936 |
| yosys-simple/multiplier.rules.json | 95a076e58da6853fe931bf1ab01585f93949e3bb0d70c03eeb5ff90d207b1b22 |
| yosys-simple/multiplier.gaps.json | 8eb46bd73faa62a039b7c80384e3d8a98c641d3817083d3fc19bd1d2e50e75d7 |
| yosys-simple/i2c_master_tests.rules.json | 570c19359fd746bb4be4acd3c8755b3ad7860b90ae92ff1580d1582db5e44a79 |
| yosys-simple/i2c_master_tests.gaps.json | 85d5e61021b3df785876169c0806a0c152abcffa57334b4003bbee757fd8139a |
| yosys-simple/undef_eqx_nex.rules.json | 7eb3183f267c85c594b7a2b64a5729f298c56c051b79e84889b58ae459791fb0 |
| yosys-simple/undef_eqx_nex.gaps.json | 9f62a7827d806e6a1aefa73fdd3a6f70a329900784ce2a4d16f2c409d71c8374 |
| yosys-simple/always03.rules.json | 2ea5b9acbdce10bcc01703d9574cb3c3b571b54151f8948758c1b90111182bd3 |
| yosys-simple/always03.gaps.json | 6466c1249247dde7e6dfc9f9e5f568b2595192b32a9d4b8ff064aaedf3167dd3 |
| yosys-simple/forloops.rules.json | 38b436e055bc50fa29c850e23e381eebcee64c60d0dcca1106fd4b1760decf8b |
| yosys-simple/forloops.gaps.json | 6b4006bed36380d6645757aab58da52937ae88e964d32efd1b8a997edddcd49f |
| yosys-simple/realexpr.rules.json | 4a69c95a9e7946347c1d9ca813a7c9bbafa02fe2f62827cfb37ee46d3db1ee8f |
| yosys-simple/realexpr.gaps.json | 2b44c982761b53808a2489d92d809317e1a488b694ec074494e982e98d93b3f2 |
| yosys-simple/subbytes.rules.json | 31825a7b9c7ebbc6313b8a83d0854f9669bd59aaaeb901e79b1495fb034f062c |
| yosys-simple/subbytes.gaps.json | 1d39b5c13ff43887691f420d0fbc9c8cd00cf0e69d413de7bf00d2648f578955 |
| yosys-simple/signedexpr.rules.json | d96d292d080c659ba619a6921644170b152703a52a1874f86a1a8e76ee860b49 |
| yosys-simple/signedexpr.gaps.json | b84800f07fc27d277f4c635bdec1af74b55aad8f454dc041f489758a627413c0 |
| yosys-simple/task_func.rules.json | 9efe9970156e0a6fb399e28056c0a0f0b49cfab2af188aceaffdc101d534061d |
| yosys-simple/task_func.gaps.json | 6785905ce1e65631e492bf1518036c853edd4010e3068623a09c41338986b4e4 |
| yosys-simple/vloghammer.rules.json | 6ad5023d6131ef35bf99008c03c04fcabfd59bdf7c8f39e2ae0eabebcb03d74b |
| yosys-simple/vloghammer.gaps.json | 737cf860a074f06826560341d9bb529584793812f4cbd27900e2bac07c078ca4 |
| yosys-simple/wreduce.rules.json | a0e71ee3f76c1a63658ad66073298da310852e4e26d0d43051b57ff50fbd00bc |
| yosys-simple/wreduce.gaps.json | ac3b24e041ca3b2c76dddc71bfaf3e22ffee03c9d864b33ad3f7c10fc7bb6f07 |
| yosys-simple/muxtree.rules.json | 19bc5e1dc4ff6ddc5d253f811841eeb591c8dd01f0e291d0e3cb3302bee2907a |
| yosys-simple/muxtree.gaps.json | ebc31a75e87a487c8377e69303c2314f744d27b8a1f4f1b727114f6a57cc0297 |
| yosys-simple/dff_init.rules.json | baa4ef31230a9f38251a60198073edcf11508f275d01fa55cf5b1b48178bfe1d |
| yosys-simple/dff_init.gaps.json | 8712ec4f0651c5a5d7b69e1053995495932fab4ffc0ff5842d9b4ea5eceb6147 |
| yosys-simple/mem2reg.rules.json | 8a25032418235e0a0c8d4f714c8300af24b60aef472d64a57886d9de8619afe2 |
| yosys-simple/mem2reg.gaps.json | 6966759645d72d6e96a8f1b47e99927a98cdb86c063644f3fd1f5796abd7fbeb |
| yosys-simple/macros.rules.json | 84b9d83760852575b131ca66b37e61312370c5490ca057b90f921a020007f42b |
| yosys-simple/macros.gaps.json | 848b6a6ccd95fb1c4e355c73fe4e51b069ebf823619f5ff73826f29af058625b |
| yosys-simple/aes_kexp128.rules.json | e97a81deed17ab085b526dedaffe6cfcba9871d0f99323b91949f5bf22b3fe3c |
| yosys-simple/aes_kexp128.gaps.json | 1742e2599f2e5ecfd416f109b6c49f81d1552afb35139f2e8d2456947afc07f7 |
| yosys-simple/carryadd.rules.json | 4236d766c9f650e7865297b00db3b1a0dbad775fd86f9fe59b9d9966e4358964 |
| yosys-simple/carryadd.gaps.json | 39769828469b2e81c27ab78942faf77dfcc2c2198886336e80f24de17c60a205 |
| yosys-simple/loops.rules.json | c659aefdc347283c2876e41da868882c77a5ccd4c572d30c11f050ace28cecd5 |
| yosys-simple/loops.gaps.json | 5b585f5e905aedfa744dd626c4d9eb73c8d36b7318042e62bb4b0aa6d13e604f |
| yosys-simple/attrib04_net_var.rules.json | 2cc5c2153d92b57e6250891bf7380187b06d9807e601683bf4f3011eec9aef9a |
| yosys-simple/attrib04_net_var.gaps.json | a356b8e27e793ac1ac6c89ae5d490b262d83dacef2d0489a7ba2c97bf10e0d7c |
| yosys-simple/omsp_dbg_uart.rules.json | 9549603a655f3b73db86066aae079d6ac612de7bee59305a75f1359efae3efe8 |
| yosys-simple/omsp_dbg_uart.gaps.json | c80722962aa96eb9d882aa3bc2ed96f6db7006e85fbc2dca1f494ac9c26d71da |
| yosys-simple/mem_arst.rules.json | 1ff0d69327a5396d9281690e35ca1e8b12588975b63dbb702ed129ba7f4bd7dc |
| yosys-simple/mem_arst.gaps.json | 6e8f3b04dd6d54e42664c054b6cc6edc6e381a3fd8fc05790944fcf20ca26626 |
| yosys-simple/forgen02.rules.json | 47240787d45b6c70ce1dc54e2bee062f54ffcef3f130f41db02288e3d72c6521 |
| yosys-simple/forgen02.gaps.json | 8dab481d0ebe58893e83fd2cdb97a48d489c82bc880afce35119c98c9f82bb91 |
| corpus/e2e/59_i2c_master/i2c_master.rules.json | 6b5a11af72f0b48ad761652cde11de9f7c6ee859d1545b8c01fb7945235839cf |
| corpus/e2e/59_i2c_master/i2c_master.gaps.json | d414618aa6cd7e3823183ed40a3736ae66ff3d2500b7ec8667d9c01295ab03fc |
| crates/spec-i2c/rtl/i2c_master.rules.json | 4f528e90dcfd6de4cfd6bbc6ca71c4956ed0460b8abd0febcc6733876c0b557b |
| crates/spec-i2c/rtl/i2c_master.gaps.json | a3b6bc98bf4200a54f11e2922543b5f98a05265ea0ebab5372496fe7d2ba9036 |
| corpus/e2e/58_xls_acc_pipe/acc_pipe.rules.json | 0c9aa006012ec51da9b6ccd2349c65bb8bb515f052c76f5bcf7fe48cdfa9c660 |
| corpus/e2e/58_xls_acc_pipe/acc_pipe.gaps.json | f5f75fb023ec811a1f5071a45e4799896f5c8c1fab3d7921dfab71f9a5923c53 |
| corpus/e2e/57_xls_add3/add3.rules.json | dadf2a13d0119f48c0462c6741d9cac39b05b3bf55b9b12f45705a990bfea648 |
| corpus/e2e/57_xls_add3/add3.gaps.json | a42dcae9a3b628b50ee3ec83cdd4cfbab1c48281fc8b4afef7b86e2f483d233c |
| corpus/fulladd/fulladd.rules.json | c47e06e9b3dd25cde69a8c4fea0766e65419887f9286e0354233a0506a9f3232 |
| corpus/fulladd/fulladd.gaps.json | 5cc22e7593c7338ebebea6ca91b912c7f883323d52f4faac950acec898a20f84 |

## demo 樣例(不屬本表)

- golden/plugin-fixtures/ 域=§17 G8 實證 demo(record-golden.mjs 錄製),
  歷史=golden/plugin-fixtures/PROVENANCE.md;run-plugin-check ④d 對其 byte-diff。

## re-record 歷史(語義事件日志;語義輪手記逐行)

- (首錄 2026-11-04,S9;F1a/F1b+F2 確定性修復後基線,§16.6 S9①對帳=全部可解釋首錄)
