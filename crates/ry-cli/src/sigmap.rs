//! SigMap 等價(kernel/sigtools.h + hashlib.h mfp 的 union-find)。
//!
//! 語義(逐字移植):
//! * `add(from, to)` 逐位:`imerge(bfi, bti)` 使 **to 側的根成為代表元**;
//!   from/to 任一為常數位 → `ipromote`(常數位永為代表元);
//!   兩側皆常數 → 不合併。
//! * `find(bit)`:不在庫中 → 原樣;否則根的 key。
//!
//! 可觀測效應僅一個:與常數合併的 wire 位,映射後印 `"0"/"1"/"x"/"z"`
//! 而非編號;wire-wire 類的代表元選擇不影響輸出(同類同編號)。

use ry_ir::{SigBit, SigSpec};

#[derive(Default)]
pub struct SigMap {
    // 索引表:SigBit → mfp 索引(idict:首見序)。
    index: Vec<(SigBit, usize)>,
    parent: Vec<usize>,
}

impl SigMap {
    pub fn new() -> Self {
        SigMap::default()
    }

    pub fn set(&mut self, module: &ry_ir::Module) {
        self.index.clear();
        self.parent.clear();
        for (lhs, rhs) in &module.connections {
            self.add(lhs, rhs);
        }
    }

    pub fn add(&mut self, from: &SigSpec, to: &SigSpec) {
        for (f, t) in from.bits.iter().zip(to.bits.iter()) {
            // lookup=取根(yosys mfp::lookup = ifind((*this)(key))):wire
            // 性判斷與合併/提升都作用在**根**上,不是原位元(兩個已各自
            // 合併到常數根的類再 connect 時,oracle 跳過合併)。
            let bfi = self.lookup_idx(f);
            let bfi = self.ifind(bfi);
            let bti = self.lookup_idx(t);
            let bti = self.ifind(bti);
            let bf_wire = matches!(self.index[bfi].0, SigBit::Wire { .. });
            let bt_wire = matches!(self.index[bti].0, SigBit::Wire { .. });
            if bf_wire || bt_wire {
                self.imerge(bfi, bti);
                if !bf_wire {
                    self.ipromote(bfi);
                }
                if !bt_wire {
                    self.ipromote(bti);
                }
            }
        }
    }

    fn lookup_idx(&mut self, bit: &SigBit) -> usize {
        if let Some((_, i)) = self.index.iter().find(|(b, _)| b == bit) {
            return *i;
        }
        let i = self.index.len();
        self.index.push((bit.clone(), i));
        self.parent.push(usize::MAX); // -1 = 無父
        i
    }

    fn ifind(&self, mut i: usize) -> usize {
        while self.parent[i] != usize::MAX {
            i = self.parent[i];
        }
        i
    }

    fn imerge(&mut self, i: usize, j: usize) {
        let ri = self.ifind(i);
        let rj = self.ifind(j);
        if ri != rj {
            self.parent[ri] = rj; // j 的根成為合併後的根
        }
    }

    fn ipromote(&mut self, i: usize) {
        let mut k = i;
        loop {
            let next = self.parent[k];
            self.parent[k] = i;
            if next == usize::MAX {
                break;
            }
            k = next;
        }
        self.parent[i] = usize::MAX;
    }

    pub fn map_bit(&self, bit: &SigBit) -> SigBit {
        if let Some((_, i)) = self.index.iter().find(|(b, _)| b == bit) {
            self.index[self.ifind(*i)].0.clone()
        } else {
            bit.clone()
        }
    }

    pub fn map(&self, sig: &SigSpec) -> SigSpec {
        SigSpec { bits: sig.bits.iter().map(|b| self.map_bit(b)).collect() }
    }

    /// SigMap::add(SigBit)(sigtools.h:342):位在庫中且其代表元為 wire 位
    /// → promote 該位為代表元(opt_clean update_assign_map 的代表元重選面)。
    pub fn promote(&mut self, bit: &SigBit) {
        let Some(i) = self.index.iter().position(|(b, _)| b == bit) else {
            return;
        };
        let root = self.ifind(i);
        if matches!(self.index[root].0, SigBit::Wire { .. }) {
            self.ipromote(i);
        }
    }
}
