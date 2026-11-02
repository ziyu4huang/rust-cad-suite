//! ASP/1 協議編解碼層(ANALYZER-SERVICE §5;D2)。
//!
//! 編解碼律是契約不是面(§3.3):本模組放 core,單測/向量/TS 對拍都消費
//! 它;server 只是第一個用戶。TS 側同律實現=`clients/analyzer-ts/src/codec.ts`,
//! 兩端一致性由共享向量文件 `clients/analyzer-ts/fixtures/codec-vectors.json`
//! 逐字節釘死(§6.3,D7)。

pub mod frame;
pub mod json;
