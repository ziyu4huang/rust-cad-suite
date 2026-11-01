# ys-batch2-anchors — 批量收编第二批锚集(起草;RYOSYS-MVP §11.4)

每例 = case.il 自含复制 + script.txt + expected 四件套(stdout/stderr/
exit/rtlil [+json]),oracle 实录管线录制(`ry-difftest --record-il
<dir>`),禁手写。錨集:

| 锚 | 输入 | script 链 | 效果面 |
|---|---|---|---|
| async2sync | mem_arst-pbo case.il | `read_rtlil case.il; proc; async2sync; check; stat; write_rtlil -readable expected.rtlil; write_json expected.json` | Replacing 22 行序 |
| pmuxtree | fsm-pbo case.il | `…; proc; opt; pmuxtree; check; stat; write_rtlil …; write_json …` | $pmux 3→0+14 $mux 建名链 |
| dffunmap | fsm-pbo case.il | `…; proc; opt; dffunmap; check; stat; write_rtlil …; write_json …` | FF 全变 $dff(6 颗) |
| setattr-set | attrib01_module-pbo case.il | `…; proc; opt; setattr -set src_sync 1; check; write_rtlil …; write_json …` | `attribute \src_sync 1` 12 处 |
| setattr-decode-err | 同上 | `…; opt; setattr -set src_sync true; check; write_rtlil …; write_json …` | exit=1+stderr `Can't decode value 'true'!` 逐字 |
| torder | obt-carryadd case.il | `read_rtlil case.il; torder; check; stat; write_rtlil …; write_json …` | module carryadd+40 `  cell ` 行 |
| edgetypes | obt-carryadd case.il | `read_rtlil case.il; edgetypes` | 8 行排序行、无 header 不占编号 |
| edgetypes-multibit | 手写最小 .il(≥1 多 bit port cell) | `read_rtlil case.il; edgetypes` | `type.port[index]` 下标形 |

-setattr 引号/-unset/-mod 形按 PROVENANCE「M3 续弧·批量收编第二批普查」
节探针定盘(slice-6 录制时落)。

**收口**:anchors 迁 corpus/ys-e2e(slice-8),SUMMARY=463+迁入数实录
定盘禁硬编;本目录在 difftest 遍历面外(无 expected.* 即跳过),起草期
零影响。
