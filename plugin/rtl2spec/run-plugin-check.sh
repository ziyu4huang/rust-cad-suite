#!/usr/bin/env bash
# run-plugin-check.sh — 插件版本一致性与拓展面检查闸(§17.6;總閘成員,bash 直跑)。
# 判据(與 §16 既有閘不重複:不跑 13 檔驗證集/不跑 MCP 全程回環/不跑 regress):
#   ①版本四點一致:manifest.version = CHANGELOG 頭 [X.Y.Z] =
#     dist/server.mjs SERVER_VERSION = serverInfo.version(引用同一常量的
#     獨立行驗證,防字面量硬編碼漂移)
#   ②manifest 靜態校驗(復用 scripts/validate-manifest.mjs)
#   ③ABI 裝載校驗:bun 裝載 dist/wasm → rtl_spec_version() ∈
#     server.mjs EXPECTED_ABI 名單(內核 ABI 軌;§17.5)
#   ③c 導出面(§19.8;C-S2):server.mjs 引用的全部 api.<fn> ⊆ dist wasm 實際
#     導出——「新 server+舊 wasm」只會在調用時失敗,此處提前(閘驗出貨物 D8 同款)
#   ④拓展面冒煙(wasm 腿=dist 同一產物):
#     正樣 gap_dead_branch.v → 恰 [G8-dead_branch-001,G8-dead_branch_else-001]
#     全 certain(集合相等斷言,§16.2 形);
#     負樣 gap_clean_branches.v → 零 items(誤報紅路面);
#     兩檔輸出與 golden/plugin-fixtures/ byte-diff(錄製=真實運行禁手寫)。
#   ⑤SSOT 漂移(§19.2 D-r1a 防回歸;scripts/ssot-drift-check.mjs;收尾弧 S4 起含 (e) RTL2SPEC-PLUGIN §20 釘選全表==
#     pinned-files.list 逐檔名對拍、(f) RTL2SPEC-FINISH-PLAN 無「[欠]」欠條):schema.rs
#     DEFAULT_MAX_RULES=N 為唯一來源——(a) dist/scripts 下 .mjs 規則提取呼叫的
#     字面量實參 ∈{0,N};(b) server.mjs 描述/rtl-rulebook SKILL 上限數字==N;
#     (c) server.mjs `const max =` 回退分支與 batch 實參=字面量 0;(d) native
#     emit.rs 引用 DEFAULT_MAX_RULES 且分派段無數字缺省。
#   ⑥skill 零語義(§19.8 RV2-14;C-S2):每個 skills/*/SKILL.md 中的 `rtl_` 前綴
#     標識符 ⊆ server 工具名(skill 只編排工具調用與解讀欄位,不發明能力)。
# 退出碼:0=全過;1=判據敗;2=環境缺(bun/wasm 產物)。
set -u
PLUGIN_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$PLUGIN_ROOT/../.." && pwd)"
cd "$REPO_ROOT"

echo "== rtl2spec run-plugin-check =="
command -v bun >/dev/null 2>&1 || { echo "FAIL guard: bun not in PATH"; exit 2; }
[ -f "$PLUGIN_ROOT/dist/wasm/rtl_spec_wasm_bg.wasm" ] || { echo "FAIL guard: dist/wasm 缺(run-rtl-validation.sh 同步)"; exit 2; }

FAIL=0
ck() { if [ "$1" = 0 ]; then echo "PASS $2"; else echo "FAIL $2"; FAIL=1; fi; }

# ① 版本四點一致
MANIFEST_V=$(python3 -c "import json;print(json.load(open('$PLUGIN_ROOT/.zcode-plugin/plugin.json'))['version'])")
CHANGELOG_V=$(grep -m1 -o '^## \[[0-9]*\.[0-9]*\.[0-9]*\]' "$PLUGIN_ROOT/CHANGELOG.md" | tr -d '[]#' | sed 's/^ //' )
SERVER_V=$(grep -m1 -o "const SERVER_VERSION = '[^']*'" "$PLUGIN_ROOT/dist/server.mjs" | sed "s/.*'\(.*\)'.*/\1/")
SERVERINFO_LINE=$(grep -n 'serverInfo:' "$PLUGIN_ROOT/dist/server.mjs" | head -1)
echo "manifest=$MANIFEST_V changelog=$CHANGELOG_V server=$SERVER_V"
[ "$MANIFEST_V" = "$CHANGELOG_V" ] && [ "$MANIFEST_V" = "$SERVER_V" ]; ck $? "①a 版本三點一致(manifest=CHANGELOG=SERVER_VERSION)"
echo "$SERVERINFO_LINE" | grep -q 'version: SERVER_VERSION'; ck $? "①b serverInfo.version 引用 SERVER_VERSION 常量(禁字面量硬編碼)"
semver_re='^[0-9]+\.[0-9]+\.[0-9]+$'
echo "$MANIFEST_V" | grep -qE "$semver_re"; ck $? "①c manifest 版本為 semver 三段"

# ② manifest 靜態校驗(既有校驗器復用;輸出含 wasm sha256 溯源)
bun "$PLUGIN_ROOT/scripts/validate-manifest.mjs" >/tmp/plugin-check-manifest.log 2>&1
ck $? "② manifest 靜態校驗(validate-manifest.mjs,詳見 /tmp/plugin-check-manifest.log)"

# ③④ ABI 裝載校驗+拓展面冒煙(bun 單進程內嵌;§17.5/§17.3)
bun - "$PLUGIN_ROOT" <<'BUN_EOF' >/tmp/plugin-check-smoke.log 2>&1
import { existsSync, readdirSync, readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
const pluginRoot = process.argv[2];
const root = pluginRoot + '/../..';
const { default: init } = await import(pluginRoot + '/dist/wasm/rtl_spec_wasm.js');
await init({ module_or_path: new Uint8Array(readFileSync(pluginRoot + '/dist/wasm/rtl_spec_wasm_bg.wasm')) });
const api = await import(pluginRoot + '/dist/wasm/rtl_spec_wasm.js');
let failures = 0;
const ck = (ok, name, detail='') => { console.log(`${ok ? 'PASS' : 'FAIL'} ${name}${detail ? ' — ' + detail : ''}`); if (!ok) failures++; };

// ③ ABI 裝載校驗:rtl_spec_version ∈ EXPECTED_ABI(從 server.mjs 提取名單)
const serverSrc = readFileSync(pluginRoot + '/dist/server.mjs', 'utf8');
const expectedAbi = serverSrc.match(/const EXPECTED_ABI = \[([^\]]*)\]/)?.[1]?.match(/'([^']+)'/g)?.map(s => s.slice(1, -1)) ?? [];
const abi = api.rtl_spec_version();
ck(expectedAbi.length > 0, '③a EXPECTED_ABI 名單非空(server.mjs)', JSON.stringify(expectedAbi));
ck(expectedAbi.includes(abi), `③b 內核 ABI=${abi} ∈ EXPECTED_ABI`, expectedAbi.join('/'));

// ③c 導出面:server.mjs 的 api.<fn> 引用 ⊆ dist wasm 實際導出(§19.8)。
const apiRefs = [...new Set([...serverSrc.matchAll(/\bapi\.([A-Za-z_][A-Za-z0-9_]*)\s*\(/g)].map((m) => m[1]))].sort();
const missingApi = apiRefs.filter((n) => typeof api[n] !== 'function');
ck(apiRefs.length >= 9, '③c-0 api 引用面非空(防空轉)', `${apiRefs.length} 個`);
ck(missingApi.length === 0, '③c server.mjs api.* ⊆ dist wasm 導出', missingApi.join(',') || `${apiRefs.length} 全在`);

// ⑥ skill 零語義:skills/*/SKILL.md 的 rtl_ 標識符 ⊆ server 工具名(§19.8)。
const toolNames = [...serverSrc.matchAll(/^\s*name:\s*'(rtl_[a-z_]+)'/gm)].map((m) => m[1]);
ck(toolNames.length >= 5, '⑥-0 工具名表非空(防空轉)', toolNames.join(','));
const skillsDir = pluginRoot + '/skills';
const skillIds = [];
for (const d of readdirSync(skillsDir).sort()) {
  const f = `${skillsDir}/${d}/SKILL.md`;
  if (!existsSync(f)) continue;
  for (const m of readFileSync(f, 'utf8').matchAll(/rtl_[a-z_]+/g)) skillIds.push([d, m[0]]);
}
const unknownIds = [...new Set(skillIds.filter(([, id]) => !toolNames.includes(id)).map(([d, id]) => `${d}:${id}`))];
ck(skillIds.length > 0, '⑥-1 skill 標識符面非空(防空轉)', `${skillIds.length} 處`);
ck(unknownIds.length === 0, '⑥ skills rtl_ 標識符 ⊆ server 工具名', unknownIds.join(',') || 'ok');

// ④ 拓展面冒煙:正樣集合相等+負樣零命中+demo golden byte-diff
const scan = (f) => api.rtl_scan_gaps_sv(f, readFileSync(root + '/' + f, 'utf8'));
const pos = 'plugin/rtl2spec/fixtures/gap_dead_branch.v';
const neg = 'plugin/rtl2spec/fixtures/gap_clean_branches.v';
const posRep = JSON.parse(scan(pos));
const ids = posRep.items.map(i => i.id).sort();
const want = ['G8-dead_branch-001', 'G8-dead_branch_else-001'];
ck(JSON.stringify(ids) === JSON.stringify(want), '④a 正樣 expected 集合相等', JSON.stringify(ids));
ck(posRep.items.length > 0 && posRep.items.every(i => i.confidence === 'certain'), '④b 正樣 confidence 全 certain');
const negRep = JSON.parse(scan(neg));
ck(negRep.items.length === 0, '④c 負樣零命中(誤報紅路面)', JSON.stringify(negRep.items.map(i => i.id)));
for (const f of [pos, neg]) {
  const stem = f.split('/').pop().replace(/\.v$/, '');
  const gold = readFileSync(`${pluginRoot}/golden/plugin-fixtures/${stem}.gaps.json`, 'utf8');
  ck(scan(f) === gold, `④d demo golden byte-diff ${stem}`);
}
process.exit(failures ? 1 : 0);
BUN_EOF
ck $? "③④ ABI 裝載校驗+拓展面冒煙(詳見 /tmp/plugin-check-smoke.log)"
grep -E '^(PASS|FAIL) ' /tmp/plugin-check-smoke.log

# ⑤ SSOT 漂移(§19.2 D-r1a;紅路演練實錄=§19 C-S1 實作記錄)
bun "$PLUGIN_ROOT/scripts/ssot-drift-check.mjs" >/tmp/plugin-check-ssot.log 2>&1
ck $? "⑤ SSOT 漂移 DEFAULT_MAX_RULES(a)..(d)+釘選全表對拍(e)+收尾弧欠條已清(f)(詳見 /tmp/plugin-check-ssot.log)"
grep -E '^(PASS|FAIL|INFO)' /tmp/plugin-check-ssot.log

if [ "$FAIL" -ne 0 ]; then echo "== run-plugin-check FAIL =="; exit 1; fi
echo "== run-plugin-check PASS =="
exit 0
