// 端到端冒煙(ANALYZER-SERVICE §6.5):對真 server 做請求/響應回環+八項斷言。
// 用法:bun smoke.ts <SOCK>(server 由 run-smoke.sh 起,本腳本只拿 socket 路徑;
// 進程退出碼斷言不在本面——R1-5,移址 analyzer-server/tests/loopback.rs)。

import * as fs from "node:fs";
import * as net from "node:net";
import {
  FrameError,
  JObj,
  decodeFrame,
  encodeFrame,
  type J,
} from "./src/codec.ts";
import { AnalyzerClient } from "./src/client.ts";
import { AspError, type AnalyzeResult } from "./src/types.ts";

function fail(msg: string): never {
  console.error(`smoke: FAIL: ${msg}`);
  process.exit(1);
}

function assert(cond: unknown, msg: string): void {
  if (!cond) fail(msg);
}

const sockPath = process.argv[2];
if (!sockPath) {
  console.error("usage: bun smoke.ts <SOCK>");
  process.exit(2);
}

const SV_OK =
  "module counter(input clk, rst_n);\n" +
  "  reg [7:0] q;\n" +
  "  wire [7:0] d;\n" +
  "  assign d = q;\n" +
  "  always @(posedge clk) q <= q + 1'b1;\n" +
  "endmodule\n";
const IL_OK =
  "autoidx 1\n" +
  "module \\top\n" +
  "  wire input 1 width 8 \\x\n" +
  "  wire output 2 width 8 \\y\n" +
  "  wire width 8 \\t\n" +
  "  connect \\t \\x\n" +
  "end\n";
const SV_BROKEN =
  "module ok(input a);\nendmodule\n" +
  "module bad(input a);\nwire ;\nassign = 1'b1;\nendmodule\n";

async function main(): Promise<void> {
  const t0 = performance.now();

  // ① 連線+握手+ping(ping → pong:true;§6.5-1)
  const c = await AnalyzerClient.connect(sockPath);
  const tFirst = performance.now();
  assert(c.serverInfo?.proto === 1, "hello handshake must report proto=1");
  assert(c.serverInfo?.server === "analyzerd", "hello must identify analyzerd");
  const pong = await c.ping();
  assert(pong.pong === true, "ping must return pong:true");
  console.log(`smoke: connect+hello ${Math.round(tFirst - t0)} ms; ping ${Math.round(performance.now() - tFirst)} ms (uptime_ms=${pong.uptime_ms})`);

  // ② SV analyze(§6.5-2)
  let tOp = performance.now();
  const sv = await c.analyze({ kind: "sv", name: "counter.v", source: SV_OK });
  assert(sv.has_errors === false, "SV_OK must be clean");
  assert(sv.modules.length === 1, "SV_OK must have 1 module");
  const m = sv.modules[0];
  assert(m.name === "counter", `module name: ${m.name}`);
  assert(m.ports.length === 2, "counter must expose 2 ports");
  assert(m.ports[0].name === "clk" && m.ports[0].dir === "input" && m.ports[0].width === 1, "clk port");
  assert(m.ports[1].name === "rst_n" && m.ports[1].dir === "input" && m.ports[1].width === 1, "rst_n port");
  const q = m.nets.find((n) => n.name === "q");
  assert(q !== undefined && q.kind === "reg" && q.width === 8, "net q = reg[7:0]");
  console.log(`smoke: sv analyze ${Math.round(performance.now() - tOp)} ms (modules=${sv.modules.length})`);

  // ③ RTLIL analyze(§6.5-3)
  tOp = performance.now();
  const il = await c.analyze({ kind: "rtlil", name: "top.il", source: IL_OK });
  assert(il.has_errors === false, "IL_OK must be clean");
  assert(il.modules.length === 1 && il.modules[0].name === "\\top", "IL_OK module \\top");
  assert(il.modules[0].ports.length === 2, "\\top must expose 2 ports");
  assert(il.modules[0].ports[0].name === "\\x" && il.modules[0].ports[0].dir === "input" && il.modules[0].ports[0].width === 8, "\\x port");
  assert(il.modules[0].ports[1].dir === "output", "\\y port");
  console.log(`smoke: rtlil analyze ${Math.round(performance.now() - tOp)} ms`);

  // ④ SV 語法錯 → has_errors:true 且 diagnostics 非空(§6.5-4)
  const broken = await c.analyze({ kind: "sv", name: "broken.v", source: SV_BROKEN });
  assert(broken.has_errors === true, "SV_BROKEN must have errors");
  assert(broken.diagnostics.length > 0, "SV_BROKEN must carry diagnostics");
  assert(broken.diags_text.length > 0, "SV_BROKEN must render diag text");
  assert(broken.modules.some((mm) => mm.name === "ok"), "partial module ok must survive");

  // ⑤ E_OP / E_ARGS(§6.5-5;raw 幀直接對 codec,連線保持)
  const raw = await rawConnect(sockPath);
  sendRaw(raw, "frobnicate", new JObj([]));
  const errResp = await recvRaw(raw);
  assert(errResp.ok === false && errResp.code === "E_OP", `unknown op must E_OP, got ${JSON.stringify(errResp)}`);
  sendRawOp(raw, new JObj([
    ["id", 2n],
    ["op", "analyze"],
    ["args", new JObj([["kind", "sv"], ["name", "x"]])], // 缺 source
  ]));
  const errResp2 = await recvRaw(raw);
  assert(errResp2.ok === false && errResp2.code === "E_ARGS", `missing source must E_ARGS, got ${JSON.stringify(errResp2)}`);
  raw.destroy();

  // ⑥ 兩連線並發各發 analyze(§6.5-6)
  const tConc = performance.now();
  const [c1, c2] = await Promise.all([AnalyzerClient.connect(sockPath), AnalyzerClient.connect(sockPath)]);
  const [r1, r2] = await Promise.all([
    c1.analyze({ kind: "sv", name: "counter.v", source: SV_OK }),
    c2.analyze({ kind: "rtlil", name: "top.il", source: IL_OK }),
  ]);
  assert(r1.modules[0]?.name === "counter", "concurrent sv result");
  assert(r2.modules[0]?.name === "\\top", "concurrent rtlil result");
  c1.close();
  c2.close();
  console.log(`smoke: concurrent 2-conn analyze ${Math.round(performance.now() - tConc)} ms`);

  // ⑦ shutdown → ack 後 socket 文件消失(§6.5-7;進程退出碼在 loopback.rs)
  const bye = await c.shutdown();
  assert(bye.bye === true, "shutdown must ack bye:true");
  const deadline = Date.now() + 3000;
  while (fs.existsSync(sockPath) && Date.now() < deadline) {
    await new Promise((r) => setTimeout(r, 10));
  }
  assert(!fs.existsSync(sockPath), "socket file must vanish after shutdown");
  c.close();

  console.log("smoke: all 8 assertion groups passed");
}

// ---- E_OP/E_ARGS 用的原始幀通道(直呼 codec,不經 client) ----

interface RawErr {
  ok: boolean;
  code: string;
}

function rawConnect(path: string): Promise<net.Socket> {
  return new Promise((resolve, reject) => {
    const s = net.createConnection(path);
    s.once("error", reject);
    s.once("connect", () => {
      s.off("error", reject);
      // 握手(§5.4)
      const hello = encodeFrame(
        new JObj([["id", 0n], ["op", "hello"], ["args", new JObj([["proto", 1n]])]]),
      );
      s.write(hello);
      s.once("data", function first() {
        s.off("data", first);
        resolve(s);
      });
    });
  });
}

function sendRaw(s: net.Socket, op: string, args: J): void {
  s.write(
    encodeFrame(
      new JObj([
        ["id", 1n],
        ["op", op],
        ["args", args],
      ]),
    ),
  );
}

function sendRawOp(s: net.Socket, obj: JObj): void {
  s.write(encodeFrame(obj));
}

async function recvRaw(s: net.Socket): Promise<RawErr> {
  const buf: Buffer = await new Promise((resolve, reject) => {
    const to = setTimeout(() => reject(new Error("raw recv timeout")), 3000);
    s.once("data", (d: Buffer) => {
      clearTimeout(to);
      resolve(d);
    });
    s.once("close", () => {
      clearTimeout(to);
      reject(new Error("raw connection closed"));
    });
  });
  const v = decodeFrame(new Uint8Array(buf));
  if (!(v instanceof JObj)) throw new Error("response not an object");
  const err = v.get("error");
  const ok = v.get("ok") === true;
  if (!ok && err instanceof JObj) {
    return { ok: false, code: typeof err.get("code") === "string" ? (err.get("code") as string) : "?" };
  }
  return { ok, code: "" };
}

main().catch((e) => {
  if (e instanceof AspError || e instanceof FrameError || e instanceof Error) {
    console.error(`smoke: FAIL: ${e.message}`);
  } else {
    console.error(`smoke: FAIL: ${e}`);
  }
  process.exit(1);
});
