// gap_hang_repro.v — F2 挂死复现形(§16.0/R16-3;S8 落地)
//
// scopes.v 死循环 bug 的最小复现:组合 always + 命名块内局部 reg +
// 自环边 + 双信号环。原 bfs_path 用 prev.contains_key 防重但 start
// 不在 prev,自环边把 prev[start] 写成 start(自映射),路径重建
// while 沿自映射永不停(native 4s+ 挂死/wasm 秒级 trap;实测图仅
// nodes=2/edges=7)。修复后必须:进程可终止、G5 报告确定(进程内
// 12 跑逐字不变)、零 trap。
//
// 用法:gap-fixture-check.mjs 守卫行(wasm 腿 12 跑 byte-identical
// + G5 类在場)与 eval 终止性防线的会触发形。
module gap_hang_repro(
    input wire a,
    input wire b,
    output reg x,
    output reg y
);
    always @* begin : blk
        reg t1;
        reg t2;
        t1 = a;
        t2 = b;
        x = t1 & t2;
        y = t1 | t2;
        x = x ^ y;  // x 自环(读自身)
        y = y ^ x;  // y 自环 + x/y 双信号环
    end
endmodule
