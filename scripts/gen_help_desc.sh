#!/bin/bash
# M3 续弧·批量收编 §10.3-4:HELP_SHORT 306 行机械生成脚本(录制序禁手抄)。
#
# 来源=oracle 实跑 `yosys -H` 的 306 条「名字+描述」行(名单行形
# `    %-20s %s`;名长>20 不截断)。录制序=-H 输出序=std::map 字典序
# (register.cc:42 pass_register 迭代序),**禁运行时排序**——生成表
# 保持录制序,消费面按序拼装。
#
# 免费对拍闸(§10.3-4):生成表按 bare 形拼装与 `-H` 输出 diff 应零差
# (P5 byte 同形)。消费面=crates/ry-cli/src/help_desc.rs(落点钉死,
# 禁落 registry.rs——表含 clean/log/help/opt_balance_tree 四名,带引号
# 元组形落 registry.rs 将命中 §10.5-4 负向 grep 恒红)。
#
# 用法:scripts/gen_help_desc.sh > crates/ry-cli/src/help_desc.rs
# (oracle 路径可经 $YOSYS 覆盖;默认=repo 内 vendor-reference 路径)
set -euo pipefail
cd "$(dirname "$0")/.."
YOSYS="${YOSYS:-vendor-reference/suite-dl/oss-cad-suite/bin/yosys}"

cat <<'HDR'
//! HELP_SHORT:yosys -H 录制的 306 条命令短描述(gen_help_desc.sh 机械生成;禁手改)。
//!
//! 来源=oracle `yosys -H` 实跑(scripts/gen_help_desc.sh,默认
//! vendor-reference/suite-dl/oss-cad-suite/bin/yosys;录制日期见 git log
//! 本檔);录制序=-H 输出序=std::map 字典序(register.cc:42),
//! **禁运行时排序**(RYOSYS-MVP §10.3-4)。消费面=bare help 名单行
//! (`    %-20s %s` 形;名对齐 20 列、名长>20 不截断)。
HDR
echo "pub const HELP_SHORT: &[(&str, &str)] = &["
"$YOSYS" -H 2>/dev/null | awk '
    /^    [A-Za-z_0-9][A-Za-z_0-9]*[ \t]/ {
        line = substr($0, 5)            # 剥前导 4 空格
        name = line; sub(/[ \t].*$/, "", name)
        desc = line; sub(/^[A-Za-z_0-9]+[ \t]+/, "", desc)
        gsub(/\\/, "\\\\", desc); gsub(/"/, "\\\"", desc)
        printf "    (\"%s\", \"%s\"),\n", name, desc
    }
' 
echo "];"
