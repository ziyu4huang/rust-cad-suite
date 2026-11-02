// ASP/1 幀 + JSON 子集編解碼,TS 側(ANALYZER-SERVICE §5/§6.2;D2/D7)。
// 與 Rust `analyzer-core/src/proto/*` 同律同禁項;一致性由共享向量文件
// `fixtures/codec-vectors.json` 逐字節釘死(雙端測試同讀)。
//
// 實現約束(§6.2,R1-7/R1-2 bun 1.4.2 實測釘死,禁慣用捷徑):
// 1. 禁 JSON.parse —— 浮點/指數/重複鍵/越 2^53 整數被其靜默放行;
//    decode = 手寫遞歸下降文本 parser(與 Rust Json::parse 同律)。
// 2. 禁 Buffer.toString('utf8') 做幀校驗 —— 非法 UTF-8 靜默替換 U+FFFD;
//    前置 = TextDecoder('utf-8', {fatal:true})(非法序列拋 TypeError)。
// 3. 禁 JSON.stringify 處理含 Int 負載 —— number 路線精度丟真、bigint
//    路線拋 TypeError;Int 恆 BigInt.toString(10)(與 Rust i64 Display
//    逐字節同形,§5.3 律 2)。
// 4. Int 型別 = bigint(唯一整數表示;型別層擋 number 混入)。
// 5. 對象表示 = JObj 包裝類(與方案 §6.2 原文 `[string, J][]` 聯合型的
//    偏差,已記入方案 §17 實作記錄:原形無法區分空對象與空數組,ping 的
//    `{}` args 無法表示;JObj 兩者皆可表)。

export const FRAME_HEADER_LEN = 4;
/** 默認幀上限 64 MiB(§5.1;與 server 同常量,兩端同值)。 */
export const MAX_FRAME_DEFAULT = 64 * 1024 * 1024;
/** 幀上限鉗位天花板 1 GiB(§5.1)。 */
export const MAX_FRAME_LIMIT = 1024 * 1024 * 1024;

const I64_MIN = -9223372036854775808n;
const I64_MAX = 9223372036854775807n;

/** 巢狀深度上限(與 Rust MAX_DEPTH 同值)。 */
const MAX_DEPTH = 128;

const fatalUtf8 = new TextDecoder("utf-8", { fatal: true });
const encoder = new TextEncoder();

/** JSON 子集值:Null/Bool=原始型、Int=bigint、Str=string、Arr=J[]、Obj=JObj。 */
export type J = null | boolean | bigint | string | J[] | JObj;

/** 保序對象:寫出序=插入序(§5.3 律 4,向量 byte-exact 的前提)。 */
export class JObj {
  readonly entries: [string, J][];
  constructor(entries: [string, J][]) {
    this.entries = entries;
  }
  get(key: string): J | undefined {
    return this.entries.find(([k]) => k === key)?.[1];
  }
}

export class CodecError extends Error {
  constructor(msg: string) {
    super(`codec: ${msg}`);
  }
}

export class FrameError extends Error {
  constructor(msg: string) {
    super(`frame: ${msg}`);
  }
}

export function clampMaxFrame(v: number): number {
  return Math.min(v, MAX_FRAME_LIMIT);
}

// ---- 寫律(§5.3;與 Rust Json::write 同形) ----

export function writeStr(s: string): string {
  let out = '"';
  for (const ch of s) {
    const cp = ch.codePointAt(0)!;
    if (ch === '"') out += '\\"';
    else if (ch === "\\") out += "\\\\";
    else if (cp === 0x08) out += "\\b";
    else if (cp === 0x0c) out += "\\f";
    else if (ch === "\n") out += "\\n";
    else if (ch === "\r") out += "\\r";
    else if (ch === "\t") out += "\\t";
    else if (cp < 0x20) out += "\\u" + cp.toString(16).padStart(4, "0");
    else out += ch;
  }
  return out + '"';
}

export function writeValue(v: J): string {
  if (v === null) return "null";
  if (typeof v === "boolean") return v ? "true" : "false";
  if (typeof v === "bigint") return v.toString(10); // §5.3 律 2
  if (typeof v === "string") return writeStr(v);
  if (v instanceof JObj) {
    return (
      "{" +
      v.entries.map(([k, val]) => writeStr(k) + ":" + writeValue(val)).join(",") +
      "}"
    );
  }
  if (Array.isArray(v)) return "[" + v.map(writeValue).join(",") + "]";
  throw new CodecError(`cannot encode value of type ${typeof v}`);
}

/** 正典序列化(緊湊文本)。 */
export function encodeText(obj: J): string {
  return writeValue(obj);
}

/** 正典序列化 → UTF-8 負載字節。 */
export function encode(obj: J): Uint8Array {
  return encoder.encode(writeValue(obj));
}

/** 編碼一幀:4 字節大端長度前綴 + 負載(單次分配)。 */
export function encodeFrame(obj: J): Uint8Array {
  const payload = encode(obj);
  const frame = new Uint8Array(FRAME_HEADER_LEN + payload.length);
  new DataView(frame.buffer).setUint32(0, payload.length, false); // big-endian
  frame.set(payload, FRAME_HEADER_LEN);
  return frame;
}

// ---- 幀讀(§5.1:先檢查後分配) ----

export function readFrameHeader(frame: Uint8Array): number {
  if (frame.length < FRAME_HEADER_LEN) {
    throw new FrameError("frame shorter than header");
  }
  return ((frame[0] << 24) | (frame[1] << 16) | (frame[2] << 8) | frame[3]) >>> 0;
}

export function checkFrameLen(len: number, max: number): void {
  if (len === 0) throw new FrameError("empty frame (len == 0)");
  if (len > max) throw new FrameError(`frame length ${len} exceeds max ${max}`);
}

/** 幀 → 負載值:前綴檢查 → 長度校驗 → fatal UTF-8 → 嚴格 JSON 子集 parse。 */
export function decodeFrame(frame: Uint8Array, maxFrame = MAX_FRAME_DEFAULT): J {
  const len = readFrameHeader(frame);
  checkFrameLen(len, maxFrame);
  const payload = frame.subarray(FRAME_HEADER_LEN);
  if (payload.length < len) throw new FrameError("truncated frame payload");
  let text: string;
  try {
    text = fatalUtf8.decode(payload.subarray(0, len));
  } catch {
    throw new FrameError("payload is not valid UTF-8");
  }
  return parse(text);
}

// ---- 嚴格 JSON 子集 parser(與 Rust Json::parse 同律同禁項) ----

class Parser {
  private c: string[];
  private pos = 0;
  private depth = 0;

  constructor(text: string) {
    // 按碼點切分(ES2015 spread 對 astral 字符產出單碼點)。
    this.c = [...text];
  }

  /** 入口:前後吃空白,中間一個值,拒尾隨內容。 */
  run(): J {
    this.ws();
    const v = this.value();
    this.ws();
    if (this.pos !== this.c.length) this.err("trailing content after JSON value");
    return v;
  }

  private err(msg: string): never {
    throw new CodecError(`${msg} at char offset ${this.pos}`);
  }

  private peek(): string | undefined {
    return this.c[this.pos];
  }

  private bump(): string | undefined {
    return this.c[this.pos++];
  }

  private ws(): void {
    while (this.peek() === " " || this.peek() === "\t" || this.peek() === "\n" || this.peek() === "\r") {
      this.pos++;
    }
  }

  private value(): J {
    const ch = this.peek();
    switch (ch) {
      case undefined:
        return this.err("unexpected end of input");
      case "{":
        return this.object();
      case "[":
        return this.array();
      case '"':
        return this.string();
      case "t":
        return this.lit("true", true);
      case "f":
        return this.lit("false", false);
      case "n":
        return this.lit("null", null);
      case "-":
      case "0":
      case "1":
      case "2":
      case "3":
      case "4":
      case "5":
      case "6":
      case "7":
      case "8":
      case "9":
        return this.number();
      default:
        this.err(`unexpected character \`${ch}\``);
    }
  }

  private lit(word: string, v: J): J {
    for (const expected of word) {
      if (this.bump() !== expected) this.err(`invalid literal, expected \`${word}\``);
    }
    return v;
  }

  private object(): J {
    if (++this.depth > MAX_DEPTH) this.err("nesting too deep");
    this.pos++; // '{'
    const entries: [string, J][] = [];
    this.ws();
    if (this.peek() === "}") {
      this.pos++;
      this.depth--;
      return new JObj(entries);
    }
    for (;;) {
      this.ws();
      if (this.peek() !== '"') this.err("expected object key string");
      const key = this.string() as string;
      this.ws();
      if (this.peek() !== ":") this.err("expected `:` after object key");
      this.pos++;
      this.ws();
      const val = this.value();
      if (entries.some(([k]) => k === key)) this.err(`duplicate object key \`${key}\``);
      entries.push([key, val]);
      this.ws();
      const ch = this.peek();
      if (ch === ",") this.pos++; // 尾逗號:下一輪鍵檢查自然拒
      else if (ch === "}") {
        this.pos++;
        break;
      } else this.err("expected `,` or `}` in object");
    }
    this.depth--;
    return new JObj(entries);
  }

  private array(): J {
    if (++this.depth > MAX_DEPTH) this.err("nesting too deep");
    this.pos++; // '['
    const items: J[] = [];
    this.ws();
    if (this.peek() === "]") {
      this.pos++;
      this.depth--;
      return items;
    }
    for (;;) {
      this.ws();
      items.push(this.value());
      this.ws();
      const ch = this.peek();
      if (ch === ",") this.pos++;
      else if (ch === "]") {
        this.pos++;
        break;
      } else this.err("expected `,` or `]` in array");
    }
    this.depth--;
    return items;
  }

  private string(): string {
    this.pos++; // '"'
    let out = "";
    for (;;) {
      const ch = this.bump();
      if (ch === undefined) this.err("unterminated string");
      if (ch === '"') return out;
      if (ch === "\\") {
        const esc = this.bump();
        switch (esc) {
          case undefined:
            this.err("unterminated escape");
          case '"':
            out += '"';
            break;
          case "\\":
            out += "\\";
            break;
          case "/":
            out += "/";
            break;
          case "b":
            out += "\b";
            break;
          case "f":
            out += "\f";
            break;
          case "n":
            out += "\n";
            break;
          case "r":
            out += "\r";
            break;
          case "t":
            out += "\t";
            break;
          case "u": {
            const hi = this.hex4();
            let cp: number;
            if (hi >= 0xd800 && hi <= 0xdbff) {
              // 高代理必須緊跟 \uDC00..\uDFFF(孤代理=拒)。
              if (this.bump() !== "\\" || this.bump() !== "u") this.err("lone high surrogate");
              const lo = this.hex4();
              if (!(lo >= 0xdc00 && lo <= 0xdfff)) this.err("invalid low surrogate");
              cp = 0x10000 + ((hi - 0xd800) << 10) + (lo - 0xdc00);
            } else if (hi >= 0xdc00 && hi <= 0xdfff) {
              this.err("lone low surrogate");
            } else {
              cp = hi;
            }
            out += String.fromCodePoint(cp);
            break;
          }
          default:
            this.err(`invalid escape \`\\${esc}\``);
        }
        continue;
      }
      const cp = ch.codePointAt(0)!;
      if (cp < 0x20) this.err("raw control character in string");
      if (cp >= 0xd800 && cp <= 0xdfff) this.err("lone surrogate in text");
      out += ch;
    }
  }

  private hex4(): number {
    let v = 0;
    for (let i = 0; i < 4; i++) {
      const ch = this.bump();
      if (ch === undefined || !/^[0-9a-fA-F]$/.test(ch)) this.err("invalid `\\u` escape");
      v = v * 16 + Number.parseInt(ch!, 16);
    }
    return v;
  }

  private number(): J {
    const start = this.pos;
    if (this.peek() === "-") this.pos++;
    const ch = this.peek();
    if (ch === "0") {
      this.pos++;
      if (this.peek() !== undefined && this.peek()! >= "0" && this.peek()! <= "9") {
        this.err("leading zero in number");
      }
    } else if (ch !== undefined && ch >= "1" && ch <= "9") {
      while (this.peek() !== undefined && this.peek()! >= "0" && this.peek()! <= "9") this.pos++;
    } else {
      this.err("invalid number");
    }
    // 浮點/指數禁項(§5.2)。
    const next = this.peek();
    if (next === "." || next === "e" || next === "E") {
      this.err("float/exponent numbers not in ASP/1 subset");
    }
    const text = this.c.slice(start, this.pos).join("");
    const b = BigInt(text);
    if (b < I64_MIN || b > I64_MAX) this.err("integer out of i64 range");
    return b;
  }
}

/** 嚴格 JSON 子集 parse:§5.2 禁項全表;未知鍵不拒、重複鍵拒。 */
export function parse(text: string): J {
  return new Parser(text).run();
}

function unhex(h: string): Uint8Array {
  const clean = h.replace(/\s+/g, "");
  if (clean.length % 2 !== 0) throw new Error("hex must be even-length");
  const out = new Uint8Array(clean.length / 2);
  for (let i = 0; i < out.length; i++) {
    out[i] = Number.parseInt(clean.slice(i * 2, i * 2 + 2), 16);
  }
  return out;
}

export { unhex };
