// TS↔Rust 編解碼一致性(ANALYZER-SERVICE §6.3,D7):與 Rust 側
// crates/analyzer-core/tests/codec_vectors.rs 讀**同一** fixtures 文件,
// 正例逐字節對齊、敵意例雙端同判「拒」。bun test 執行(run-smoke.sh 內建步,
// R1-3:向量斷言進固定閘)。
//
// 敵意用例(§8.1 codec.test.ts 行)在此另有內聯補充:深度上限/尾隨內容/
// 孤代理文本。

import { describe, expect, test } from "bun:test";
import vectors from "../fixtures/codec-vectors.json";
import {
  CodecError,
  FrameError,
  JObj,
  MAX_FRAME_DEFAULT,
  type J,
  checkFrameLen,
  decodeFrame,
  encodeFrame,
  readFrameHeader,
  unhex,
} from "../src/codec.ts";

function hex(b: Uint8Array): string {
  return Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");
}

/** 向量文件本身是可信本地數據(import 型 JSON),非線路流量。 */
const list: { name: string; kind: string; frame_hex: string; note: string }[] =
  vectors.vectors;

describe("codec vectors", () => {
  test("surface coverage (§6.3 清單,≥12 條)", () => {
    expect(list.length).toBeGreaterThanOrEqual(12);
    for (const required of [
      "hello_request",
      "analyze_request_nested",
      "str_all_seven_escapes",
      "ctrl_char_lowercase_u",
      "cjk_astral_raw",
      "i64_min",
      "i64_max",
      "error_frame",
      "oversize_len_prefix",
      "empty_len_frame",
      "float_payload_rejected",
      "duplicate_key_rejected",
      "non_utf8_payload_rejected",
    ]) {
      expect(list.some((v) => v.name === required)).toBe(true);
    }
  });

  test("canonical: decode → encode 逐字節回放", () => {
    for (const v of list.filter((x) => x.kind === "canonical")) {
      const frame = unhex(v.frame_hex);
      const val = decodeFrame(frame); // 不拋=幀頭/長度/UTF-8/JSON 全過
      expect(encodeFrame(val)).toEqual(frame); // 整幀重編碼逐字節相等
      const len = readFrameHeader(frame);
      expect(len).toBe(frame.length - 4); // 長度前綴=負載長
      expect(len).toBeLessThanOrEqual(MAX_FRAME_DEFAULT);
    }
  });

  test("reject_codec: parser 禁項雙端同判(§5.2)", () => {
    for (const v of list.filter((x) => x.kind === "reject_codec")) {
      const frame = unhex(v.frame_hex);
      expect(() => decodeFrame(frame)).toThrow(CodecError);
    }
  });

  test("reject_frame: 幀頭級違約先於分配(§5.1)", () => {
    for (const v of list.filter((x) => x.kind === "reject_frame")) {
      const frame = unhex(v.frame_hex);
      const len = readFrameHeader(frame);
      expect(() => checkFrameLen(len, MAX_FRAME_DEFAULT)).toThrow(FrameError);
      expect(() => decodeFrame(frame)).toThrow(FrameError);
    }
  });

  test("reject_utf8: fatal decoder 拒非法序列(§6.2 約束 2)", () => {
    for (const v of list.filter((x) => x.kind === "reject_utf8")) {
      const frame = unhex(v.frame_hex);
      expect(() => decodeFrame(frame)).toThrow(FrameError);
    }
  });

  test("i64_min:Int=bigint 精確往返(R1-2;number 路線已實測丟真禁入)", () => {
    const v = list.find((x) => x.name === "i64_min")!;
    const frame = unhex(v.frame_hex);
    const val = decodeFrame(frame) as JObj;
    expect(val.entries[0][0]).toBe("n");
    expect(val.entries[0][1]).toBe(-9223372036854775808n);
    // bigint 編碼路線復現同一幀(BigInt.toString(10) 與 Rust i64 Display 同形)。
    expect(encodeFrame(new JObj([["n", -9223372036854775808n]]))).toEqual(frame);
  });

  test("hello_request 結構斷言(Int 解碼=bigint)", () => {
    const v = list.find((x) => x.name === "hello_request")!;
    const req = decodeFrame(unhex(v.frame_hex)) as JObj;
    expect(req.get("id")).toBe(1n);
    expect(req.get("op")).toBe("hello");
    expect((req.get("args") as JObj).get("proto")).toBe(1n);
  });
});

describe("codec hostile inline cases", () => {
  test("孤代理轉義文本拒收(原始字節路徑;TextEncoder 對孤代理是無聲 U+FFFD 替換,編碼路徑產不出該形,故手工構幀)", () => {
    // payload = {"s":"\ud800x"}(高代理未跟 \u)→ 全 ASCII 字節。
    const payload = new TextEncoder().encode('{"s":"\\ud800x"}');
    const frame = new Uint8Array(4 + payload.length);
    new DataView(frame.buffer).setUint32(0, payload.length, false);
    frame.set(payload, 4);
    expect(() => decodeFrame(frame)).toThrow(CodecError);
  });

  test("深度上限:129 層拒收、128 層恰收(base [] 即第 1 層)", () => {
    let over: J = [];
    for (let i = 0; i < 128; i++) over = [over]; // 128 次包裹 → 129 層
    expect(() => decodeFrame(encodeFrame(over))).toThrow(CodecError);
    let ok: J = [];
    for (let i = 0; i < 127; i++) ok = [ok]; // 127 次包裹 → 128 層
    expect(decodeFrame(encodeFrame(ok))).toEqual(ok);
  });

  test('尾隨內容拒收({"a":1}x)', () => {
    // payload = {"a":1}x → 7b 22 61 22 3a 31 7d 78。
    expect(() => decodeFrame(unhex("000000087b2261223a317d78"))).toThrow(CodecError);
  });

  test("number 混入型別層擋不住的 number 編碼拋錯(Int 恆 bigint)", () => {
    // 運行時防線:number 不是合法 J 的 Int 形——writeValue 拋 CodecError。
    expect(() => encodeFrame([1] as unknown as J)).toThrow(CodecError);
  });

  test("幀短於幀頭拒收", () => {
    expect(() => readFrameHeader(unhex("0000"))).toThrow(FrameError);
  });
});
