#!/bin/bash
# 批量收编第二批:六命令 help 全文常量生成(oracle 实跑转录,禁手写;
# RYOSYS-MVP §11.3-8)。用法:bash scripts/gen_help_batch2.sh
# 产出片断打印到 stdout,供 help_desc.rs 落档(形态=RAW r#"…"# 字面)。
set -euo pipefail
YOSYS="${YOSYS:-vendor-reference/suite-dl/oss-cad-suite/bin/yosys}"
for name in async2sync pmuxtree dffunmap setattr torder edgetypes; do
    echo "=== $name ==="
    "$YOSYS" -p "help $name" 2>/dev/null \
        | awk '/^-- Running command/{on=1;next} /^End of script\./{on=0} on'
done
