//! # ry-ir — RTLIL 對象模型 + parser + `write_rtlil -readable` 打印機
//!
//! ryosys 的 IR 主權層(docs/RYOSYS-MVP.md §1.2;ADR-0010 草稿)。
//! 輸入形態=oracle 產出的 RTLIL 文本(-readable 傳統全名形);詞法與
//! 語法逐字照抄 yosys `rtlil_frontend.cc`(行導向、ID 吃到空白、名字可含
//! `[]:'=.`、切片前必須有空格——`\bus[7:4]` 是**單個 ID** 會走
//! "Wire not found" 硬錯),傾印逐形照抄 `rtlil_backend.cc`。
//!
//! M0 邊界(鐵則 3:未支援構造=帶里程碑指針硬錯,禁靜默跑偏):
//! * `twines` 區塊 / `$priv@` / `$pub@` 名 token → milestone Y1
//!   (oracle 對 $priv@ 是靜默錯名,猜=假綠);
//! * cell `parameter real`、sigspec 內字串常數 → milestone Y1;
//! * `write_rtlil` 默認壓縮形不在本 crate(只有 readable 形)。
//!
//! 零外部依賴;插入序容器自帶(oracle dict 傾印=插入序、insert 首值保留)。
//!
//! ```
//! let design = ry_ir::parse_rtlil("module \\top\n  wire \\a\nend\n").unwrap();
//! let text = ry_ir::write_rtlil_readable(&design);
//! // roundtrip 冪等:再解析再傾印,剝首行後 byte-exact。
//! let d2 = ry_ir::parse_rtlil(&text).unwrap();
//! assert_eq!(ry_ir::write_rtlil_readable(&d2), text);
//! ```

pub mod ast;
pub mod lexer;
pub mod parser;
pub mod printer;

pub use ast::{
    Bit, CaseRule, Cell, Const, Design, Id, InsertMap, MemWriteAction, Memory, Module, Process,
    SigBit, SigSpec, SyncRule, SyncType, SwitchRule, Wire,
};
pub use lexer::RyError;
pub use parser::parse_design as parse_rtlil;
pub use printer::write_readable as write_rtlil_readable;
