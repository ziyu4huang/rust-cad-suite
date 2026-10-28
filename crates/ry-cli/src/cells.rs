//! known-cell 端口方向表(kernel/newcelltypes.h StaticCellTypes 家族
//! 的機械轉錄;check 邊界的量化錨點=corpus/ys-e2e/PROVENANCE 的
//! 語料 cell 普查表——錄製輪遇新類型=擴表同輪修 check)。
//!
//! 消費面:`check`(未知內部類型判定+輸入/輸出位歸帳)與
//! `write_json` 的 port_directions(僅 known cell 印出)。

/// (類型, 輸入端口序, 輸出端口序)。
pub const CELLS: &[(&str, &[&str], &[&str])] = &[
    ("$not", &["A"], &["Y"]),
    ("$pos", &["A"], &["Y"]),
    ("$buf", &["A"], &["Y"]),
    ("$neg", &["A"], &["Y"]),
    ("$reduce_and", &["A"], &["Y"]),
    ("$reduce_or", &["A"], &["Y"]),
    ("$reduce_xor", &["A"], &["Y"]),
    ("$reduce_xnor", &["A"], &["Y"]),
    ("$reduce_bool", &["A"], &["Y"]),
    ("$logic_not", &["A"], &["Y"]),
    ("$slice", &["A"], &["Y"]),
    ("$lut", &["A"], &["Y"]),
    ("$sop", &["A"], &["Y"]),
    ("$and", &["A", "B"], &["Y"]),
    ("$or", &["A", "B"], &["Y"]),
    ("$xor", &["A", "B"], &["Y"]),
    ("$xnor", &["A", "B"], &["Y"]),
    ("$shl", &["A", "B"], &["Y"]),
    ("$shr", &["A", "B"], &["Y"]),
    ("$sshl", &["A", "B"], &["Y"]),
    ("$sshr", &["A", "B"], &["Y"]),
    ("$shift", &["A", "B"], &["Y"]),
    ("$shiftx", &["A", "B"], &["Y"]),
    ("$lt", &["A", "B"], &["Y"]),
    ("$le", &["A", "B"], &["Y"]),
    ("$eq", &["A", "B"], &["Y"]),
    ("$ne", &["A", "B"], &["Y"]),
    ("$eqx", &["A", "B"], &["Y"]),
    ("$nex", &["A", "B"], &["Y"]),
    ("$ge", &["A", "B"], &["Y"]),
    ("$gt", &["A", "B"], &["Y"]),
    ("$add", &["A", "B"], &["Y"]),
    ("$sub", &["A", "B"], &["Y"]),
    ("$mul", &["A", "B"], &["Y"]),
    ("$div", &["A", "B"], &["Y"]),
    ("$mod", &["A", "B"], &["Y"]),
    ("$divfloor", &["A", "B"], &["Y"]),
    ("$modfloor", &["A", "B"], &["Y"]),
    ("$pow", &["A", "B"], &["Y"]),
    ("$logic_and", &["A", "B"], &["Y"]),
    ("$logic_or", &["A", "B"], &["Y"]),
    ("$concat", &["A", "B"], &["Y"]),
    ("$macc", &["A", "B"], &["Y"]),
    ("$bweqx", &["A", "B"], &["Y"]),
    ("$mux", &["A", "B", "S"], &["Y"]),
    ("$pmux", &["A", "B", "S"], &["Y"]),
    ("$bwmux", &["A", "B", "S"], &["Y"]),
    ("$bmux", &["A", "S"], &["Y"]),
    ("$demux", &["A", "S"], &["Y"]),
    ("$lcu", &["P", "G", "CI"], &["CO"]),
    ("$alu", &["A", "B", "CI", "BI"], &["X", "Y", "CO"]),
    ("$macc_v2", &["A", "B", "C"], &["Y"]),
    ("$fa", &["A", "B", "C"], &["X", "Y"]),
    ("$sr", &["SET", "CLR"], &["Q"]),
    ("$ff", &["D"], &["Q"]),
    ("$dff", &["CLK", "D"], &["Q"]),
    ("$dffe", &["CLK", "EN", "D"], &["Q"]),
    ("$dffsr", &["CLK", "SET", "CLR", "D"], &["Q"]),
    ("$dffsre", &["CLK", "SET", "CLR", "D", "EN"], &["Q"]),
    ("$adff", &["CLK", "ARST", "D"], &["Q"]),
    ("$adffe", &["CLK", "ARST", "D", "EN"], &["Q"]),
    ("$aldff", &["CLK", "ALOAD", "AD", "D"], &["Q"]),
    ("$aldffe", &["CLK", "ALOAD", "AD", "D", "EN"], &["Q"]),
    ("$sdff", &["CLK", "SRST", "D"], &["Q"]),
    ("$sdffe", &["CLK", "SRST", "D", "EN"], &["Q"]),
    ("$sdffce", &["CLK", "SRST", "D", "EN"], &["Q"]),
    ("$dlatch", &["EN", "D"], &["Q"]),
    ("$adlatch", &["EN", "D", "ARST"], &["Q"]),
    ("$dlatchsr", &["EN", "SET", "CLR", "D"], &["Q"]),
    ("$anyinit", &["D"], &["Q"]),
    ("$memrd", &["CLK", "EN", "ADDR"], &["DATA"]),
    ("$memrd_v2", &["CLK", "EN", "ARST", "SRST", "ADDR"], &["DATA"]),
    ("$memwr", &["CLK", "EN", "ADDR", "DATA"], &[]),
    ("$memwr_v2", &["CLK", "EN", "ADDR", "DATA"], &[]),
    ("$meminit", &["ADDR", "DATA"], &[]),
    ("$meminit_v2", &["ADDR", "DATA", "EN"], &[]),
    ("$mem", &["RD_CLK", "RD_EN", "RD_ADDR", "WR_CLK", "WR_EN", "WR_ADDR", "WR_DATA"], &["RD_DATA"]),
    ("$mem_v2", &["RD_CLK", "RD_EN", "RD_ARST", "RD_SRST", "RD_ADDR", "WR_CLK", "WR_EN", "WR_ADDR", "WR_DATA"], &["RD_DATA"]),
    ("$fsm", &["CLK", "ARST", "CTRL_IN"], &["CTRL_OUT"]),
    ("$tribuf", &["A", "EN"], &["Y"]),
    ("$barrier", &["A"], &["Y"]),
    ("$assert", &["A", "EN"], &[]),
    ("$assume", &["A", "EN"], &[]),
    ("$live", &["A", "EN"], &[]),
    ("$fair", &["A", "EN"], &[]),
    ("$cover", &["A", "EN"], &[]),
    ("$initstate", &[], &["Y"]),
    ("$anyconst", &[], &["Y"]),
    ("$anyseq", &[], &["Y"]),
    ("$allconst", &[], &["Y"]),
    ("$allseq", &[], &["Y"]),
    ("$equiv", &["A", "B"], &["Y"]),
    ("$specify2", &["EN", "SRC", "DST"], &[]),
    ("$specify3", &["EN", "SRC", "DST", "DAT"], &[]),
    ("$specrule", &["SRC_EN", "DST_EN", "SRC", "DST"], &[]),
    ("$print", &["EN", "ARGS", "TRG"], &[]),
    ("$check", &["A", "EN", "ARGS", "TRG"], &[]),
    ("$set_tag", &["A", "SET", "CLR"], &["Y"]),
    ("$get_tag", &["A"], &["Y"]),
    ("$overwrite_tag", &["A", "SET", "CLR"], &[]),
    ("$original_tag", &["A"], &["Y"]),
    ("$future_ff", &["A"], &["Y"]),
    ("$scopeinfo", &[], &[]),
    ("$input_port", &[], &["Y"]),
    ("$connect", &["A", "B"], &[]),
    ("$_TBUF_", &["A", "E"], &["Y"]),
    ("$_BUF_", &["A"], &["Y"]),
    ("$_NOT_", &["A"], &["Y"]),
    ("$_AND_", &["A", "B"], &["Y"]),
    ("$_NAND_", &["A", "B"], &["Y"]),
    ("$_OR_", &["A", "B"], &["Y"]),
    ("$_NOR_", &["A", "B"], &["Y"]),
    ("$_XOR_", &["A", "B"], &["Y"]),
    ("$_XNOR_", &["A", "B"], &["Y"]),
    ("$_ANDNOT_", &["A", "B"], &["Y"]),
    ("$_ORNOT_", &["A", "B"], &["Y"]),
    ("$_MUX_", &["A", "B", "S"], &["Y"]),
    ("$_NMUX_", &["A", "B", "S"], &["Y"]),
    ("$_AOI3_", &["A", "B", "C"], &["Y"]),
    ("$_OAI3_", &["A", "B", "C"], &["Y"]),
    ("$_AOI4_", &["A", "B", "C", "D"], &["Y"]),
    ("$_OAI4_", &["A", "B", "C", "D"], &["Y"]),
    ("$_MUX4_", &["A", "B", "C", "D", "S", "T"], &["Y"]),
    ("$_MUX8_", &["A", "B", "C", "D", "E", "F", "G", "H", "S", "T", "U"], &["Y"]),
    ("$_MUX16_", &["A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "S", "T", "U", "V"], &["Y"]),
    ("$_SR_NN_", &["S", "R"], &["Q"]),
    ("$_SR_NP_", &["S", "R"], &["Q"]),
    ("$_SR_PN_", &["S", "R"], &["Q"]),
    ("$_SR_PP_", &["S", "R"], &["Q"]),
    ("$_FF_", &["D"], &["Q"]),
    ("$_DFF_N_", &["C", "D"], &["Q"]),
    ("$_DLATCH_N_", &["E", "D"], &["Q"]),
    ("$_DFFE_NN_", &["C", "D", "E"], &["Q"]),
    ("$_ALDFF_NN_", &["C", "L", "AD", "D"], &["Q"]),
    ("$_DFF_NN0_", &["C", "R", "D"], &["Q"]),
    ("$_SDFF_NN0_", &["C", "R", "D"], &["Q"]),
    ("$_DLATCH_NN0_", &["E", "R", "D"], &["Q"]),
    ("$_DFFE_NN0N_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFE_NN0N_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFCE_NN0N_", &["C", "R", "D", "E"], &["Q"]),
    ("$_DFFE_NN0P_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFE_NN0P_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFCE_NN0P_", &["C", "R", "D", "E"], &["Q"]),
    ("$_DFF_NN1_", &["C", "R", "D"], &["Q"]),
    ("$_SDFF_NN1_", &["C", "R", "D"], &["Q"]),
    ("$_DLATCH_NN1_", &["E", "R", "D"], &["Q"]),
    ("$_DFFE_NN1N_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFE_NN1N_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFCE_NN1N_", &["C", "R", "D", "E"], &["Q"]),
    ("$_DFFE_NN1P_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFE_NN1P_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFCE_NN1P_", &["C", "R", "D", "E"], &["Q"]),
    ("$_ALDFFE_NNN_", &["C", "L", "AD", "D", "E"], &["Q"]),
    ("$_DFFSR_NNN_", &["C", "S", "R", "D"], &["Q"]),
    ("$_DFFSRE_NNNN_", &["C", "S", "R", "D", "E"], &["Q"]),
    ("$_DFFSRE_NNNP_", &["C", "S", "R", "D", "E"], &["Q"]),
    ("$_ALDFFE_NNP_", &["C", "L", "AD", "D", "E"], &["Q"]),
    ("$_DFFSR_NNP_", &["C", "S", "R", "D"], &["Q"]),
    ("$_DFFSRE_NNPN_", &["C", "S", "R", "D", "E"], &["Q"]),
    ("$_DFFSRE_NNPP_", &["C", "S", "R", "D", "E"], &["Q"]),
    ("$_DLATCHSR_NNN_", &["E", "S", "R", "D"], &["Q"]),
    ("$_DLATCHSR_NNP_", &["E", "S", "R", "D"], &["Q"]),
    ("$_DFFE_NP_", &["C", "D", "E"], &["Q"]),
    ("$_ALDFF_NP_", &["C", "L", "AD", "D"], &["Q"]),
    ("$_DFF_NP0_", &["C", "R", "D"], &["Q"]),
    ("$_SDFF_NP0_", &["C", "R", "D"], &["Q"]),
    ("$_DLATCH_NP0_", &["E", "R", "D"], &["Q"]),
    ("$_DFFE_NP0N_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFE_NP0N_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFCE_NP0N_", &["C", "R", "D", "E"], &["Q"]),
    ("$_DFFE_NP0P_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFE_NP0P_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFCE_NP0P_", &["C", "R", "D", "E"], &["Q"]),
    ("$_DFF_NP1_", &["C", "R", "D"], &["Q"]),
    ("$_SDFF_NP1_", &["C", "R", "D"], &["Q"]),
    ("$_DLATCH_NP1_", &["E", "R", "D"], &["Q"]),
    ("$_DFFE_NP1N_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFE_NP1N_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFCE_NP1N_", &["C", "R", "D", "E"], &["Q"]),
    ("$_DFFE_NP1P_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFE_NP1P_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFCE_NP1P_", &["C", "R", "D", "E"], &["Q"]),
    ("$_ALDFFE_NPN_", &["C", "L", "AD", "D", "E"], &["Q"]),
    ("$_DFFSR_NPN_", &["C", "S", "R", "D"], &["Q"]),
    ("$_DFFSRE_NPNN_", &["C", "S", "R", "D", "E"], &["Q"]),
    ("$_DFFSRE_NPNP_", &["C", "S", "R", "D", "E"], &["Q"]),
    ("$_ALDFFE_NPP_", &["C", "L", "AD", "D", "E"], &["Q"]),
    ("$_DFFSR_NPP_", &["C", "S", "R", "D"], &["Q"]),
    ("$_DFFSRE_NPPN_", &["C", "S", "R", "D", "E"], &["Q"]),
    ("$_DFFSRE_NPPP_", &["C", "S", "R", "D", "E"], &["Q"]),
    ("$_DLATCHSR_NPN_", &["E", "S", "R", "D"], &["Q"]),
    ("$_DLATCHSR_NPP_", &["E", "S", "R", "D"], &["Q"]),
    ("$_DFF_P_", &["C", "D"], &["Q"]),
    ("$_DLATCH_P_", &["E", "D"], &["Q"]),
    ("$_DFFE_PN_", &["C", "D", "E"], &["Q"]),
    ("$_ALDFF_PN_", &["C", "L", "AD", "D"], &["Q"]),
    ("$_DFF_PN0_", &["C", "R", "D"], &["Q"]),
    ("$_SDFF_PN0_", &["C", "R", "D"], &["Q"]),
    ("$_DLATCH_PN0_", &["E", "R", "D"], &["Q"]),
    ("$_DFFE_PN0N_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFE_PN0N_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFCE_PN0N_", &["C", "R", "D", "E"], &["Q"]),
    ("$_DFFE_PN0P_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFE_PN0P_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFCE_PN0P_", &["C", "R", "D", "E"], &["Q"]),
    ("$_DFF_PN1_", &["C", "R", "D"], &["Q"]),
    ("$_SDFF_PN1_", &["C", "R", "D"], &["Q"]),
    ("$_DLATCH_PN1_", &["E", "R", "D"], &["Q"]),
    ("$_DFFE_PN1N_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFE_PN1N_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFCE_PN1N_", &["C", "R", "D", "E"], &["Q"]),
    ("$_DFFE_PN1P_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFE_PN1P_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFCE_PN1P_", &["C", "R", "D", "E"], &["Q"]),
    ("$_ALDFFE_PNN_", &["C", "L", "AD", "D", "E"], &["Q"]),
    ("$_DFFSR_PNN_", &["C", "S", "R", "D"], &["Q"]),
    ("$_DFFSRE_PNNN_", &["C", "S", "R", "D", "E"], &["Q"]),
    ("$_DFFSRE_PNNP_", &["C", "S", "R", "D", "E"], &["Q"]),
    ("$_ALDFFE_PNP_", &["C", "L", "AD", "D", "E"], &["Q"]),
    ("$_DFFSR_PNP_", &["C", "S", "R", "D"], &["Q"]),
    ("$_DFFSRE_PNPN_", &["C", "S", "R", "D", "E"], &["Q"]),
    ("$_DFFSRE_PNPP_", &["C", "S", "R", "D", "E"], &["Q"]),
    ("$_DLATCHSR_PNN_", &["E", "S", "R", "D"], &["Q"]),
    ("$_DLATCHSR_PNP_", &["E", "S", "R", "D"], &["Q"]),
    ("$_DFFE_PP_", &["C", "D", "E"], &["Q"]),
    ("$_ALDFF_PP_", &["C", "L", "AD", "D"], &["Q"]),
    ("$_DFF_PP0_", &["C", "R", "D"], &["Q"]),
    ("$_SDFF_PP0_", &["C", "R", "D"], &["Q"]),
    ("$_DLATCH_PP0_", &["E", "R", "D"], &["Q"]),
    ("$_DFFE_PP0N_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFE_PP0N_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFCE_PP0N_", &["C", "R", "D", "E"], &["Q"]),
    ("$_DFFE_PP0P_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFE_PP0P_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFCE_PP0P_", &["C", "R", "D", "E"], &["Q"]),
    ("$_DFF_PP1_", &["C", "R", "D"], &["Q"]),
    ("$_SDFF_PP1_", &["C", "R", "D"], &["Q"]),
    ("$_DLATCH_PP1_", &["E", "R", "D"], &["Q"]),
    ("$_DFFE_PP1N_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFE_PP1N_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFCE_PP1N_", &["C", "R", "D", "E"], &["Q"]),
    ("$_DFFE_PP1P_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFE_PP1P_", &["C", "R", "D", "E"], &["Q"]),
    ("$_SDFFCE_PP1P_", &["C", "R", "D", "E"], &["Q"]),
    ("$_ALDFFE_PPN_", &["C", "L", "AD", "D", "E"], &["Q"]),
    ("$_DFFSR_PPN_", &["C", "S", "R", "D"], &["Q"]),
    ("$_DFFSRE_PPNN_", &["C", "S", "R", "D", "E"], &["Q"]),
    ("$_DFFSRE_PPNP_", &["C", "S", "R", "D", "E"], &["Q"]),
    ("$_ALDFFE_PPP_", &["C", "L", "AD", "D", "E"], &["Q"]),
    ("$_DFFSR_PPP_", &["C", "S", "R", "D"], &["Q"]),
    ("$_DFFSRE_PPPN_", &["C", "S", "R", "D", "E"], &["Q"]),
    ("$_DFFSRE_PPPP_", &["C", "S", "R", "D", "E"], &["Q"]),
    ("$_DLATCHSR_PPN_", &["E", "S", "R", "D"], &["Q"]),
    ("$_DLATCHSR_PPP_", &["E", "S", "R", "D"], &["Q"]),
];

/// 類型是否為已知內建 cell(cell_known)。
pub fn cell_known(t: &str) -> bool {
    CELLS.iter().any(|(ty, _, _)| *ty == t)
}

/// cell_evaluable(kernel/celltypes.h;名單=newcelltypes.h
/// setup_internals_eval ∪ setup_stdcells_eval 的 is_evaluable 旗標集)。
/// 環偵測(check)的**參與條件**之一——不 evaluable 且非 ff/mem/barrier
/// 的 cell 完全不進邊圖(check.cc:352;實錄:$sdff 誤走 fallback 全連通
/// 把 next-state 回授誤判成組合環,16 案例 stdout 紅,2026-10-27)。
pub fn cell_evaluable(t: &str) -> bool {
    const EVAL: &[&str] = &[
        // setup_internals_eval:unary
        "$not", "$pos", "$buf", "$neg", "$reduce_and", "$reduce_or",
        "$reduce_xor", "$reduce_xnor", "$reduce_bool", "$logic_not",
        "$slice", "$lut", "$sop",
        // binary
        "$and", "$or", "$xor", "$xnor", "$shl", "$shr", "$sshl", "$sshr",
        "$shift", "$shiftx", "$lt", "$le", "$eq", "$ne", "$eqx", "$nex",
        "$ge", "$gt", "$add", "$sub", "$mul", "$div", "$mod", "$divfloor",
        "$modfloor", "$pow", "$logic_and", "$logic_or", "$concat", "$macc",
        "$bweqx",
        // select
        "$mux", "$pmux", "$bwmux", "$bmux", "$demux",
        // 其它內建
        "$lcu", "$alu", "$macc_v2", "$fa",
        // setup_stdcells_eval(閘級 19 種)
        "$_BUF_", "$_NOT_", "$_AND_", "$_NAND_", "$_OR_", "$_NOR_",
        "$_XOR_", "$_XNOR_", "$_ANDNOT_", "$_ORNOT_", "$_MUX_", "$_NMUX_",
        "$_MUX4_", "$_MUX8_", "$_MUX16_", "$_AOI3_", "$_OAI3_", "$_AOI4_",
        "$_OAI4_",
    ];
    EVAL.contains(&t)
}

/// is_builtin_ff(kernel/rtlil.cc:4519;名單=newcelltypes.h
/// setup_internals_ff ∪ setup_stdcells_ff 的 is_ff 旗標集)。
/// 閘級家族用前綴展開——setup_stdcells_ff 的顯式表全部形如
/// `$_<FAM>_<NP01 組合>_`($_FF_ 無後綴),後綴字元受限於 {N,P,0,1},
/// 語料外雜名(如 $_DFFX_)不會被誤配。
pub fn cell_is_builtin_ff(t: &str) -> bool {
    const FF: &[&str] = &[
        "$sr", "$ff", "$dff", "$dffe", "$dffsr", "$dffsre", "$adff",
        "$adffe", "$aldff", "$aldffe", "$sdff", "$sdffe", "$sdffce",
        "$dlatch", "$adlatch", "$dlatchsr",
    ];
    if FF.contains(&t) {
        return true;
    }
    if !t.starts_with("$_") || !t.ends_with('_') {
        return false;
    }
    const GATE_FF_FAMS: &[&str] = &[
        "$_SR_", "$_FF_", "$_DFFE_", "$_ALDFFE_", "$_DFFSR_", "$_DFFSRE_",
        "$_SDFFE_", "$_SDFFCE_", "$_DLATCHSR_", "$_DLATCH_", "$_ALDFF_",
        "$_SDFF_", "$_DFF_",
    ];
    let Some(fam) = GATE_FF_FAMS.iter().find(|f| t.starts_with(**f)) else {
        return false;
    };
    let suffix = &t[fam.len()..];
    let bare = suffix.strip_suffix('_').unwrap_or(suffix);
    if fam == &"$_FF_" {
        return suffix.is_empty();
    }
    !bare.is_empty() && bare.chars().all(|c| matches!(c, 'N' | 'P' | '0' | '1'))
}

/// 端口方向(未知類型/未知端口一律 (false, false))。
///
/// 端口名比較在 **unescape 域**:cell 連接裡的端口名是 `\Y` 轉義形
/// (yosys 內部 ID::Y 即 `\Y`),表條目是 `Y` 裸名——直接字串比較會
/// 全部落空(values 例 $pmux 的 \Y 驅動歸帳个失、check 假報 29 條
/// undriven 警告,實錄 2026-10-27)。
pub fn port_dir(t: &str, port: &str) -> (bool, bool) {
    let port = crate::jsonout::unescape(port);
    for (ty, ins, outs) in CELLS {
        if *ty == t {
            return (ins.contains(&port), outs.contains(&port));
        }
    }
    (false, false)
}
