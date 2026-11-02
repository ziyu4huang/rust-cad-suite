//! ASP/1 幀層(ANALYZER-SERVICE §5.1):4 字節大端 u32 長度前綴 + 負載。
//! 違約處置=讀到前綴即檢查、先於任何分配;超限幀不讀負載不分配。

use std::fmt;

/// 幀頭長度(字節)。
pub const HEADER_LEN: usize = 4;
/// 默認幀上限 64 MiB(§5.1;`--max-frame` 可調)。
pub const MAX_FRAME_DEFAULT: u32 = 64 * 1024 * 1024;
/// 幀上限鉗位天花板 1 GiB(§5.1)。
pub const MAX_FRAME_LIMIT: u32 = 1024 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FrameError {
    /// `len == 0` = 協議違約。
    Empty,
    /// `len > max` = 協議違約。
    TooLarge { len: u32, max: u32 },
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameError::Empty => write!(f, "empty frame (len == 0)"),
            FrameError::TooLarge { len, max } => write!(f, "frame length {len} exceeds max {max}"),
        }
    }
}

impl std::error::Error for FrameError {}

/// `--max-frame` 值鉗位(上限 1 GiB)。
pub fn clamp_max_frame(v: u32) -> u32 {
    v.min(MAX_FRAME_LIMIT)
}

/// 解析幀頭長度前綴(大端)。
pub fn header_len(header: &[u8; HEADER_LEN]) -> u32 {
    u32::from_be_bytes(*header)
}

/// 讀到前綴即檢查(先於任何分配,§5.1)。
pub fn check_len(len: u32, max: u32) -> Result<(), FrameError> {
    if len == 0 {
        Err(FrameError::Empty)
    } else if len > max {
        Err(FrameError::TooLarge { len, max })
    } else {
        Ok(())
    }
}

/// 編碼一幀:4 字節 BE 前綴 + 負載,追加寫入 `out`。
pub fn write_frame(payload: &[u8], out: &mut Vec<u8>) {
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(payload);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_len_prefix() {
        let payload = b"hello";
        let mut buf = Vec::new();
        write_frame(payload, &mut buf);
        assert_eq!(buf.len(), HEADER_LEN + 5);
        let (header, rest) = buf.split_first_chunk::<4>().unwrap();
        assert_eq!(header_len(header), 5);
        assert_eq!(rest, payload);
    }

    #[test]
    fn big_endian_order() {
        let mut buf = Vec::new();
        write_frame(&[0xaa; 300], &mut buf);
        assert_eq!(&buf[..4], &[0x00, 0x00, 0x01, 0x2c]); // 300 = 0x12c BE
    }

    #[test]
    fn reject_empty_and_oversize() {
        assert_eq!(check_len(0, MAX_FRAME_DEFAULT), Err(FrameError::Empty));
        assert_eq!(
            check_len(MAX_FRAME_DEFAULT + 1, MAX_FRAME_DEFAULT),
            Err(FrameError::TooLarge { len: MAX_FRAME_DEFAULT + 1, max: MAX_FRAME_DEFAULT })
        );
        assert_eq!(check_len(MAX_FRAME_DEFAULT, MAX_FRAME_DEFAULT), Ok(()));
        assert_eq!(header_len(&[0xff, 0xff, 0xff, 0xff]), u32::MAX);
    }

    #[test]
    fn clamp_ceiling() {
        assert_eq!(clamp_max_frame(1024), 1024);
        assert_eq!(clamp_max_frame(u32::MAX), MAX_FRAME_LIMIT);
        assert_eq!(clamp_max_frame(MAX_FRAME_DEFAULT), MAX_FRAME_DEFAULT);
    }
}
