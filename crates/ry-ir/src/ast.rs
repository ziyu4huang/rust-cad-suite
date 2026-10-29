//! RTLIL 對象模型(docs/RYOSYS-MVP.md §1.2「一次到位」的全實體語法面)。
//!
//! 迭代語義對齊 oracle:module 內 wires/memories/cells/processes 與 design 內
//! modules 按「插入序」傾印(實測 oracle dict 正向迭代=逆插入序、dump 走
//! `reversed()`=插入序,rtlil_backend.cc:320/327/334/341/395);connect 語句
//! 是 vector,傾印=出現序(rtlil_backend.cc:349)。同名 key 的 insert 語義
//! =首值保留、位置保留(std::insert 語義;實測 2026-10-27:`attribute \a 1/
//! attribute \a 3` 回寫 `attribute \a 1`)。

/// 逐字 ID:反引號(`\name`)或 `$` 開頭,名字可含 `[]:'=.` 等字元。
/// 詞法上吃到空白(見 lexer::Cursor::parse_id);本 crate 不做 unescape。
pub type Id = String;

/// 4-state+2 位元值(IEEE 語義之外的 RTLIL 擴充:`m`=memory 未解析、`-`=don't care)。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Bit {
    S0,
    S1,
    Sx,
    Sz,
    Sm,
    Sa,
}

impl Bit {
    /// dump_const 的單字元形(rtlil_backend.cc:75-81)。
    pub fn as_char(self) -> char {
        match self {
            Bit::S0 => '0',
            Bit::S1 => '1',
            Bit::Sx => 'x',
            Bit::Sz => 'z',
            Bit::Sm => 'm',
            Bit::Sa => '-',
        }
    }
}

/// 常數模型:字串域與位域兩個 variant(oracle `RTLIL::Const` 的 union 同構)。
///
/// * 位域 `bits` 為 LSB-first(oracle parse_const 先按文字序收集再 reverse,
///   rtlil_frontend.cc:301-302)。
/// * `signed` 僅在屬性/參數域保留;sigspec 域 parse 後即丟(實測 `4's0001`
///   經 connect 回寫 `4'0001`,RYOSYS-MVP §6-R7)。
/// * `unsized` 對應 CONST_FLAG_UNSIZED,dump 時不印 `N'` 前綴。
#[derive(Clone, Debug, PartialEq)]
pub enum Const {
    Str(String),
    Bits {
        width: i64,
        bits: Vec<Bit>,
        signed: bool,
    is_unsized: bool,
    },
}

impl Default for Const {
    /// 零寬全-x 常數(dump 形 `0'x`;memwr priority_mask 的常見形)。
    fn default() -> Self {
        Const::Bits { width: 0, bits: Vec::new(), signed: false, is_unsized: false }
    }
}

impl Const {
    /// 32-bit 全定值常數(oracle `RTLIL::Const(long long)` 預設寬 32)。
    pub fn from_i64_i32_width(val: i64) -> Const {
        let mut bits = Vec::with_capacity(32);
        for i in 0..32 {
            bits.push(if (val >> i) & 1 == 0 { Bit::S0 } else { Bit::S1 });
        }
        Const::Bits { width: 32, bits, signed: false, is_unsized: false }
    }

    /// 全-x 判定(is_fully_undef_x_only:所有位皆 Sx;空=true,
    /// kernel/rtlil.cc dump_const 走 `x` 短寫的條件)。
    pub fn is_fully_undef_x_only(&self) -> bool {
        match self {
            Const::Str(s) => s.is_empty(),
            Const::Bits { bits, .. } => bits.iter().all(|&b| b == Bit::Sx),
        }
    }

    pub fn width(&self) -> i64 {
        match self {
            Const::Str(_) => 0,
            Const::Bits { width, .. } => *width,
        }
    }
}

/// 插入序 map:oracle `dict` 的傾印語義(reversed 迭代=插入序)+
/// `insert` 首值保留語義。M0 不需要按 key 查找的熱路徑,線性掃即可。
#[derive(Clone, Debug, PartialEq)]
pub struct InsertMap<V> {
    pub entries: Vec<(Id, V)>,
}

impl<V> Default for InsertMap<V> {
    fn default() -> Self {
        InsertMap { entries: Vec::new() }
    }
}

impl<V> InsertMap<V> {
    pub fn new() -> Self {
        InsertMap { entries: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn get(&self, key: &str) -> Option<&V> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    /// std::unordered_map::insert 語義:已存在則**不**覆蓋(實測 2026-10-27)。
    pub fn insert(&mut self, key: Id, value: V) {
        if !self.contains(&key) {
            self.entries.push((key, value));
        }
    }

    /// dict operator[] 賦值語義:已存在=覆蓋值、位置不動;不存在=尾插
    /// (proc_init 寫 wireinit、proc_rmdead 的 set_bool_attribute 等用)。
    pub fn set(&mut self, key: Id, value: V) {
        if let Some(e) = self.entries.iter_mut().find(|(k, _)| *k == key) {
            e.1 = value;
        } else {
            self.entries.push((key, value));
        }
    }

    /// hashlib dict erase 語義(kernel/hashlib.h):swap-and-pop——把尾
    /// 元素移到被刪位。**剩餘元素順序會變**(dump 序外生面,proc 弧用)。
    pub fn swap_pop_remove(&mut self, key: &str) -> bool {
        if let Some(i) = self.entries.iter().position(|(k, _)| k == key) {
            let last = self.entries.len() - 1;
            self.entries.swap(i, last);
            self.entries.pop();
            true
        } else {
            false
        }
    }
}

/// SigSpec 的單一位:wire 位(帶 wire 名與該 wire 內的 bit offset)或常數位。
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SigBit {
    Wire { wire: Id, offset: i64 },
    Const(Bit),
}

/// 信號規格:LSB-first 的位序列(oracle SigSpec 的 BITS 形)。
///
/// chunk 化(相鄰合併)是**傾印期**的視圖(見 printer::sig_chunks),
/// 規則抄 kernel/rtlil.h `Chunks::const_iterator::next_chunk_bits`:
/// 值序上連續、物理 offset 遞增連續的 wire 位合併;相鄰常數位合併。
/// 實測錨:`{ \bus [0] \bus [1] \bus [3:2] }` 保持三 chunk(物理序與值序
/// 相反不合併)、`{ 1'0 1'1 }` 合併成 `2'01`。
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SigSpec {
    pub bits: Vec<SigBit>,
}

impl SigSpec {
    pub fn size(&self) -> i64 {
        self.bits.len() as i64
    }

    pub fn has_const(&self) -> bool {
        self.bits.iter().any(|b| matches!(b, SigBit::Const(_)))
    }

    /// oracle `SigSpec::extract(offset, count)`(值序切片;chunk 重建在傾印期)。
    pub fn extract(&self, offset: i64, count: i64) -> SigSpec {
        let start = offset.max(0) as usize;
        let end = (offset + count).max(0) as usize;
        SigSpec { bits: self.bits[start.min(self.bits.len())..end.min(self.bits.len())].to_vec() }
    }
}

/// design 頂層:autoidx 計數(read 時取 max、write 時回放,實測值逐字保留)
/// + 模組插入序清單。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Design {
    pub autoidx: i64,
    pub modules: Vec<Module>,
}

impl Design {
    pub fn find_module(&self, name: &str) -> Option<&Module> {
        self.modules.iter().find(|m| m.name == name)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Module {
    pub name: Id,
    pub attributes: InsertMap<Const>,
    /// `parameter \p`(無預設值)的宣告序清單(dump 遍歷 avail_parameters)。
    pub avail_parameters: Vec<Id>,
    pub parameter_defaults: InsertMap<Const>,
    pub wires: Vec<Wire>,
    pub memories: Vec<Memory>,
    pub cells: Vec<Cell>,
    pub processes: Vec<Process>,
    /// connect 語句=vector,傾印=出現序(非 dict)。
    pub connections: Vec<(SigSpec, SigSpec)>,
}

impl Module {
    pub fn find_wire(&self, name: &str) -> Option<&Wire> {
        self.wires.iter().find(|w| w.name == name)
    }
    pub fn find_cell(&self, name: &str) -> Option<&Cell> {
        self.cells.iter().find(|c| c.name == name)
    }
    pub fn find_memory(&self, name: &str) -> Option<&Memory> {
        self.memories.iter().find(|m| m.name == name)
    }
    pub fn find_process(&self, name: &str) -> Option<&Process> {
        self.processes.iter().find(|p| p.name == name)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Wire {
    pub name: Id,
    pub width: i64,
    pub start_offset: i64,
    pub port_id: i64,
    pub port_input: bool,
    pub port_output: bool,
    pub upto: bool,
    pub is_signed: bool,
    pub attributes: InsertMap<Const>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Memory {
    pub name: Id,
    pub width: i64,
    pub start_offset: i64,
    pub size: i64,
    pub attributes: InsertMap<Const>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cell {
    pub ctype: Id,
    pub name: Id,
    pub attributes: InsertMap<Const>,
    pub parameters: InsertMap<Const>,
    /// cell 端口連接:宣告序;重複端口名是 parse 硬錯(非 insert 覆蓋)。
    pub connections: Vec<(Id, SigSpec)>,
}

impl Cell {
    pub fn has_port(&self, name: &str) -> bool {
        self.connections.iter().any(|(p, _)| p == name)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Process {
    pub name: Id,
    pub attributes: InsertMap<Const>,
    pub root_case: CaseRule,
    pub syncs: Vec<SyncRule>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CaseRule {
    pub attributes: InsertMap<Const>,
    /// switch 的 case 比較項(root case 無 compare;傾印時 ` , ` 連接)。
    pub compare: Vec<SigSpec>,
    pub actions: Vec<(SigSpec, SigSpec)>,
    pub switches: Vec<SwitchRule>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SwitchRule {
    pub attributes: InsertMap<Const>,
    pub signal: SigSpec,
    pub cases: Vec<CaseRule>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncType {
    Low,
    High,
    Posedge,
    Negedge,
    Edge,
    Always,
    Global,
    Init,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SyncRule {
    pub stype: SyncType,
    /// always/global/init 無信號(Option);low/high/posedge/negedge/edge 必有。
    pub signal: Option<SigSpec>,
    pub actions: Vec<(SigSpec, SigSpec)>,
    pub mem_writes: Vec<MemWriteAction>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MemWriteAction {
    pub attributes: InsertMap<Const>,
    pub memid: Id,
    pub address: SigSpec,
    pub data: SigSpec,
    pub enable: SigSpec,
    pub priority_mask: Const,
}
