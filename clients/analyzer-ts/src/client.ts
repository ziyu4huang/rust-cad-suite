// AnalyzerClient — ASP/1 TS 客戶端(ANALYZER-SERVICE §6.2)。
// node:net UDS;單連線內請求排隊(響應按 id 配對,幀序=請求序,與 §5.7
// 串行契約互為鏡像);編解碼全部走 codec.ts(同律禁項,§6.2 實現約束)。

import * as net from "node:net";
import {
  CodecError,
  FrameError,
  FRAME_HEADER_LEN,
  JObj,
  MAX_FRAME_DEFAULT,
  clampMaxFrame,
  decodeFrame,
  encodeFrame,
  type J,
} from "./codec.ts";
import {
  AspError,
  type AnalyzeRequest,
  type AnalyzeResult,
  type CellInfo,
  type Diagnostic,
  type DiagnosticNote,
  type ModuleInfo,
  type NetInfo,
  type ParamInfo,
  type PortInfo,
  type ServerInfo,
  type Span,
} from "./types.ts";

export interface ClientOpts {
  /** 默認 64 MiB=server 同常量(§6.2;覆蓋僅測試用,生產路徑禁單端漂移)。 */
  maxFrame?: number;
}

interface Pending {
  resolve: (v: JObj) => void;
  reject: (e: Error) => void;
}

export class AnalyzerClient {
  private sock: net.Socket;
  private maxFrame: number;
  private nextId = 1;
  private pending = new Map<number, Pending>();
  private buffer: Buffer = Buffer.alloc(0);

  private constructor(sock: net.Socket, maxFrame: number) {
    this.sock = sock;
    this.maxFrame = maxFrame;
    sock.on("data", (d: Buffer) => this.onData(d));
    sock.on("error", (e: Error) => this.failAll(e));
    sock.on("close", () => this.failAll(new Error("connection closed")));
  }

  /** 連線 + 握手(首幀 hello,§5.4;proto 協商失敗=AspError(E_VERSION))。 */
  static async connect(path: string, opts?: ClientOpts): Promise<AnalyzerClient> {
    const maxFrame =
      opts?.maxFrame !== undefined ? clampMaxFrame(opts.maxFrame) : MAX_FRAME_DEFAULT;
    const sock = net.createConnection(path);
    await new Promise<void>((resolve, reject) => {
      const onErr = (e: Error) => {
        sock.destroy();
        reject(e);
      };
      sock.once("error", onErr);
      sock.once("connect", () => {
        sock.off("error", onErr);
        resolve();
      });
    });
    const client = new AnalyzerClient(sock, maxFrame);
    // 握手:hello + args.proto=1(§5.4)。
    const result = await client.request("hello", new JObj([["proto", 1n]]));
    const info = decodeServerInfo(result);
    if (info.proto !== 1) {
      sock.destroy();
      throw new AspError("E_VERSION", `server protocol ${info.proto}, client wants 1`);
    }
    client.serverInfo = info;
    return client;
  }

  /** 握手響應的 server 檔案(proto/server/version/ops)。 */
  serverInfo: ServerInfo | undefined;

  // ---- ops(§6.2 簽名面) ----

  async ping(): Promise<{ pong: true; uptime_ms: number }> {
    const r = await this.request("ping", new JObj([]));
    return { pong: true, uptime_ms: asNum(r.get("uptime_ms"), "result.uptime_ms") };
  }

  async analyze(req: AnalyzeRequest): Promise<AnalyzeResult> {
    const args: [string, J][] = [
      ["kind", req.kind],
      ["name", req.name],
      ["source", req.source],
    ];
    if (req.include_dirs !== undefined) args.push(["include_dirs", req.include_dirs.map((s) => s)]);
    if (req.defines !== undefined) args.push(["defines", req.defines.map((s) => s)]);
    const r = await this.request("analyze", new JObj(args));
    return decodeAnalyzeResult(r);
  }

  async shutdown(): Promise<{ bye: true }> {
    const r = await this.request("shutdown", new JObj([]));
    if (r.get("bye") !== true) throw new CodecError("shutdown ack missing bye=true");
    return { bye: true };
  }

  /** 本地關閉,不送幀(§6.2)。 */
  close(): void {
    this.sock.destroy();
  }

  // ---- 幀收發內部 ----

  private request(op: string, args: JObj): Promise<JObj> {
    if (this.sock.destroyed) return Promise.reject(new Error("client closed"));
    const id = this.nextId++;
    // 超限對稱律(R1-12):client 在寫出前先拒(拋 RangeError 不發幀)。
    const frame = encodeFrame(
      new JObj([
        ["id", BigInt(id)],
        ["op", op],
        ["args", args],
      ]),
    );
    if (frame.length - FRAME_HEADER_LEN > this.maxFrame) {
      return Promise.reject(
        new RangeError(`request frame ${frame.length - 4}B exceeds client max ${this.maxFrame}B`),
      );
    }
    return new Promise<JObj>((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      // 單次 write(4B 前綴+負載一次系統調用,§6.2)。
      this.sock.write(frame, (e) => {
        if (e) {
          this.pending.delete(id);
          reject(e);
        }
      });
    });
  }

  private onData(d: Buffer): void {
    this.buffer = Buffer.concat([this.buffer, d]);
    for (;;) {
      if (this.buffer.length < FRAME_HEADER_LEN) return;
      const len = this.buffer.readUInt32BE(0);
      if (len === 0 || len > this.maxFrame) {
        // 幀級違約(§5.1 對稱):全 pending 拒絕並斷連。
        this.failAll(new FrameError(`server frame length ${len} violates max ${this.maxFrame}`));
        this.sock.destroy();
        return;
      }
      if (this.buffer.length < FRAME_HEADER_LEN + len) return;
      const frame = this.buffer.subarray(0, FRAME_HEADER_LEN + len);
      this.buffer = this.buffer.subarray(FRAME_HEADER_LEN + len) as Buffer;
      this.dispatchFrame(frame);
    }
  }

  private dispatchFrame(frame: Uint8Array): void {
    let v: J;
    try {
      v = decodeFrame(frame, this.maxFrame);
    } catch (e) {
      this.failAll(e instanceof Error ? e : new Error(String(e)));
      this.sock.destroy();
      return;
    }
    if (!(v instanceof JObj)) {
      this.failAll(new CodecError("response is not an object"));
      return;
    }
    const id = typeof v.get("id") === "bigint" ? Number(v.get("id")) : 0;
    const p = this.pending.get(id);
    if (!p) return; // 不認識的 id(違約幀回聲)→ 丟棄
    this.pending.delete(id);
    if (v.get("ok") === true) {
      const result = v.get("result");
      p.resolve(result instanceof JObj ? result : new JObj([]));
    } else {
      const err = v.get("error");
      if (err instanceof JObj) {
        p.reject(
          new AspError(
            typeof err.get("code") === "string" ? (err.get("code") as string) : "E_INTERNAL",
            typeof err.get("message") === "string" ? (err.get("message") as string) : "",
          ),
        );
      } else {
        p.reject(new AspError("E_INTERNAL", "error frame missing error object"));
      }
    }
  }

  private failAll(e: Error): void {
    for (const [, p] of this.pending) p.reject(e);
    this.pending.clear();
  }
}

// ---- 視圖解碼(線路 bigint → 視圖 number;型別面嚴檢) ----

export function decodeServerInfo(r: JObj): ServerInfo {
  const ops: string[] = [];
  const rawOps = r.get("ops");
  if (Array.isArray(rawOps)) {
    for (const o of rawOps) {
      if (typeof o === "string") ops.push(o);
    }
  }
  return {
    proto: typeof r.get("proto") === "bigint" ? Number(r.get("proto")) : 0,
    server: typeof r.get("server") === "string" ? (r.get("server") as string) : "",
    version: typeof r.get("version") === "string" ? (r.get("version") as string) : "",
    ops,
  };
}

function asStr(v: J | undefined, what: string): string {
  if (typeof v === "string") return v;
  throw new CodecError(`${what} must be a string`);
}

function asBool(v: J | undefined, what: string): boolean {
  if (typeof v === "boolean") return v;
  throw new CodecError(`${what} must be a boolean`);
}

function asNum(v: J | undefined, what: string): number {
  if (typeof v === "bigint") return Number(v);
  throw new CodecError(`${what} must be an integer`);
}

function asArr(v: J | undefined, what: string): J[] {
  if (Array.isArray(v)) return v;
  throw new CodecError(`${what} must be an array`);
}

function asObjs(v: J | undefined, what: string): JObj[] {
  return asArr(v, what).map((x, i) => {
    if (!(x instanceof JObj)) throw new CodecError(`${what}[${i}] must be an object`);
    return x;
  });
}

function asSpan(v: J | undefined): Span | null {
  if (v === null || v === undefined) return null;
  if (!(v instanceof JObj)) throw new CodecError("span must be object or null");
  return {
    file: asStr(v.get("file"), "span.file"),
    start_line: asNum(v.get("start_line"), "span.start_line"),
    start_col: asNum(v.get("start_col"), "span.start_col"),
    end_line: asNum(v.get("end_line"), "span.end_line"),
    end_col: asNum(v.get("end_col"), "span.end_col"),
  };
}

function decodeModule(m: JObj): ModuleInfo {
  const params: ParamInfo[] = asObjs(m.get("params"), "module.params").map((p) => ({
    name: asStr(p.get("name"), "param.name"),
    value: asStr(p.get("value"), "param.value"),
  }));
  const ports: PortInfo[] = asObjs(m.get("ports"), "module.ports").map((p) => {
    const dir = asStr(p.get("dir"), "port.dir");
    if (dir !== "input" && dir !== "output" && dir !== "inout") {
      throw new CodecError(`port.dir invalid: ${dir}`);
    }
    const out: PortInfo = {
      name: asStr(p.get("name"), "port.name"),
      dir,
      width: asNum(p.get("width"), "port.width"),
      signed: asBool(p.get("signed"), "port.signed"),
      span: asSpan(p.get("span")),
    };
    const parametric = p.get("parametric");
    if (parametric === true) out.parametric = true;
    return out;
  });
  const nets: NetInfo[] = asObjs(m.get("nets"), "module.nets").map((n) => ({
    name: asStr(n.get("name"), "net.name"),
    kind: asStr(n.get("kind"), "net.kind"),
    width: asNum(n.get("width"), "net.width"),
    is_port: asBool(n.get("is_port"), "net.is_port"),
    unpacked_dims: asNum(n.get("unpacked_dims"), "net.unpacked_dims"),
    span: asSpan(n.get("span")),
  }));
  const cells: CellInfo[] = asObjs(m.get("cells"), "module.cells").map((c) => ({
    name: asStr(c.get("name"), "cell.name"),
    ctype: asStr(c.get("ctype"), "cell.ctype"),
    port_count: asNum(c.get("port_count"), "cell.port_count"),
    span: asSpan(c.get("span")),
  }));
  return {
    name: asStr(m.get("name"), "module.name"),
    params,
    ports,
    nets,
    cells,
    proc_count: asNum(m.get("proc_count"), "module.proc_count"),
    assign_count: asNum(m.get("assign_count"), "module.assign_count"),
    span: asSpan(m.get("span")),
  };
}

function decodeDiagnostics(v: J | undefined): Diagnostic[] {
  return asObjs(v, "result.diagnostics").map((d) => {
    const severity = asStr(d.get("severity"), "diag.severity");
    if (severity !== "error" && severity !== "warning" && severity !== "note") {
      throw new CodecError(`diag.severity invalid: ${severity}`);
    }
    const notes: DiagnosticNote[] = asObjs(d.get("notes"), "diag.notes").map((n) => ({
      message: asStr(n.get("message"), "note.message"),
      span: asSpan(n.get("span")),
    }));
    return {
      severity,
      code: asStr(d.get("code"), "diag.code"),
      message: asStr(d.get("message"), "diag.message"),
      span: asSpan(d.get("span")),
      notes,
    };
  });
}

function decodeAnalyzeResult(r: JObj): AnalyzeResult {
  return {
    has_errors: asBool(r.get("has_errors"), "result.has_errors"),
    modules: asObjs(r.get("modules"), "result.modules").map(decodeModule),
    diagnostics: decodeDiagnostics(r.get("diagnostics")),
    diags_text: asStr(r.get("diags_text"), "result.diags_text"),
  };
}
