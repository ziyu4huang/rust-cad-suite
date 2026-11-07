// proto-template.mjs — CONFORMANCE-PROTOCOL 模板塊硬同步(收尾弧 S0;協議 §6)。純函數,零 IO。
// 消費者=dleg-prompt.mjs(實例化取最新版)與 conformance-eval.mjs selfcheck(每次核對)。
//
// 圍欄形:`<!-- template-block vN begin -->`+```text 塊+`<!-- template-block vN end -->`(append-only,舊版保留)。
// 內容錨形:`<!-- template-sha vN <sha256> -->`(sha256=圍欄內各行以 \n 連接再加尾 \n,與 §19.15 R1-10
// 的 awk/sed/shasum 管線同值)。
// 硬同步=每版圍欄完整、版本號從 1 連續、每版恰一錨且重算相等,任一不符 throw(呼叫端 exit≠0)
// ——舊版被動過或新版未登記都是協議事件缺口,不是「取得到就用」。圍欄/錨標記只認整行,行內提及不觸發。
import { createHash } from 'node:crypto';

const BEGIN = /^<!-- template-block v([0-9]+) begin -->$/;
const ANCHOR = /^<!-- template-sha v([0-9]+) ([0-9a-f]{64}) -->$/;

export const blockSha = (lines) => createHash('sha256').update(lines.join('\n') + '\n').digest('hex');

/// 全部版本(版本號升序):[{version, lines, sha256}];任一硬同步條件不符 throw。
export function parseTemplates(protoText) {
  const lines = protoText.split('\n');
  const blocks = new Map();
  const anchors = new Map();
  lines.forEach((l, i) => {
    const a = l.match(ANCHOR);
    if (a) {
      const v = Number(a[1]);
      if (anchors.has(v)) throw new Error(`模板內容錨 v${v} 重複`);
      anchors.set(v, a[2]);
      return;
    }
    const m = l.match(BEGIN);
    if (!m) return;
    const v = Number(m[1]);
    if (blocks.has(v)) throw new Error(`模板圍欄 v${v} 重複`);
    const s = lines.indexOf('```text', i);
    const e = s < 0 ? -1 : lines.indexOf('```', s + 1);
    const end = lines.indexOf(`<!-- template-block v${v} end -->`, i);
    if (s < 0 || e < 0 || end < 0 || e > end) throw new Error(`模板圍欄 v${v} 不完整`);
    blocks.set(v, lines.slice(s + 1, e));
  });
  if (blocks.size === 0) throw new Error('模板圍欄缺');
  const versions = [...blocks.keys()].sort((x, y) => x - y);
  versions.forEach((v, k) => {
    if (v !== k + 1) throw new Error(`模板版本不連續: ${versions.join(',')}`);
  });
  for (const v of anchors.keys()) if (!blocks.has(v)) throw new Error(`內容錨 v${v} 無對應圍欄`);
  return versions.map((v) => {
    const sha256 = blockSha(blocks.get(v));
    const reg = anchors.get(v);
    if (!reg) throw new Error(`模板 v${v} 未登記內容錨`);
    if (reg !== sha256) throw new Error(`模板 v${v} 內容錨不符: 登記 ${reg.slice(0, 12)}… 重算 ${sha256.slice(0, 12)}…`);
    return { version: v, lines: blocks.get(v), sha256 };
  });
}

/// 最高版本(實例化唯一入口;舊版只作對照,禁回退取用)。
export function latestTemplate(protoText) {
  const all = parseTemplates(protoText);
  return all[all.length - 1];
}

/// 模板 §1 硬性要求第 2 條的 WAIVE 標籤枚舉(tblint WAIVE_LABELS 的逐集合對照面)。
export function waiveLabelsOf(lines) {
  const m = lines.join('\n').match(/WAIVE: <rule_id>\s+<([a-z|-]+)>/);
  if (!m) throw new Error('模板缺 WAIVE 標籤枚舉');
  return m[1].split('|');
}
