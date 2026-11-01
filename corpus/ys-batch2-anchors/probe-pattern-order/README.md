# probe-pattern-order — opt_dff pattern_t 内序二进制仲裁探针(2026-11-02 slice-2)

case.il=read_verilog(t.v)+proc+opt_clean 产物(oracle 生成);oracle-o.il=
oracle `opt_dff` 后 write_rtlil 实录。判据=两个 width-2 $ne 的 \A 向量:

* pattern {sa:1,sb:0}(DFS 插入序 [sa,sb])→ oracle `\A { \sa \sb }`
  (=插入序,intern 序 [sb,sa] 被证伪);
* pattern {sa:0,sc:0}(DFS 插入序 [sa,sc])→ oracle `\A { \sc \sa }`
  (=逆插序,插入序 [sa,sc] 被证伪);
* 另 sincos 实录 {procmux$107_CMP, start}(autoidx 名先于 parser 名=负
  index 先=raw index 律,但该律又与上述两点不符)。

四律(intern 升序/lt_by_name 字典序/正插/逆插)无一同时解释三点 ⇒
pattern_t 迭代序=hash 桶序(二进制内部态,D-ys8c 面1 同族新子面),
禁硬磕;我方 Vec DFS 确定序维持。探针命令:
`yosys -p "read_rtlil case.il; opt_dff; write_rtlil -readable o.il"`
