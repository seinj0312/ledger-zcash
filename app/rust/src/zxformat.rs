#![allow(non_snake_case)]

use bs58::decode::Error;
use core::fmt::{self, Write};
use libc::{c_char, c_int, c_uint};
use nom::error::ParseError;

use crate::parser::ParserError;

pub const MAX_NUM_STR_BUFF_LEN: usize = 30;

pub struct Writer<'a> {
    buf: &'a mut [u8],
    pub(crate) offset: usize,
}

impl<'a> Writer<'a> {
    pub fn new(buf: &'a mut [u8]) -> Self {
        Writer { buf, offset: 0 }
    }
}

impl<'a> fmt::Write for Writer<'a> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        let remainder = &mut self.buf[self.offset..];

        if remainder.len() < bytes.len() {
            return Err(core::fmt::Error);
            // overflow wit zero decimals
        }
        let remainder = &mut remainder[..bytes.len()];
        remainder.copy_from_slice(bytes);
        // Update offset to avoid overwriting
        self.offset += bytes.len();

        Ok(())
    }
}

#[no_mangle]
pub extern "C" fn rs_fpuint64_to_str(out: *mut c_char, outLen: u16, value: u64, decimals: u8) {
    let output = unsafe { core::slice::from_raw_parts_mut(out as *mut u8, outLen as usize) };

    let res = fpu64_to_str(output, value, decimals);
    if res.is_err() && outLen >= 3 {
        //(&mut output[..3]).copy_from_slice(b"ERR".as_ref());
        output.copy_from_slice(b"ERR".as_ref());
    }
}

#[no_mangle]
pub extern "C" fn rs_fpint64_to_str(out: *mut c_char, outLen: u16, value: i64, decimals: u8) {
    let output = unsafe { core::slice::from_raw_parts_mut(out as *mut u8, outLen as usize) };

    let res = fpi64_to_str(output, value, decimals);
    if res.is_err() && outLen >= 3 {
        output.copy_from_slice(b"ERR".as_ref());
    }
}

macro_rules! num_to_str {
    ($name: ident, $number: ty) => {
        pub fn $name(output: &mut [u8], number: $number) -> Result<usize, ParserError> {
            if output.len() < 2 {
                return Err(ParserError::parser_unexpected_buffer_end);
            }
            let mut writer = Writer::new(output);
            let _ = write!(writer, "{}", number)
                .map_err(|_| ParserError::parser_unexpected_buffer_end)?;
            Ok(writer.offset)
        }
    };
}

num_to_str!(u64_to_str, u64);
num_to_str!(i64_to_str, i64);

/// Fixed point u64 number
///
/// Converts an u64 number into its fixed point string representation
/// using #decimals padding zeros
/// # Arguments
/// * * `out`: the output buffer where the conversion result is written
/// * `value`: The number to convert to
/// * `decimals`: the number of decimals after the decimal point
/// # Returns
/// The number of bytes written if success or Error otherwise
pub fn fpu64_to_str(out: &mut [u8], value: u64, decimals: u8) -> Result<usize, ParserError> {
    let mut temp = [0u8; MAX_NUM_STR_BUFF_LEN];
    let len = u64_to_str(temp.as_mut(), value)?;
    fpstr_to_str(out, &temp[..len], decimals)
}

/// Fixed point i64 number
///
/// Converts an u64 number into its fixed point string representation
/// using decimals padding zeros
/// # Arguments
/// * * `out`: the output buffer where the conversion result is written
/// * `value`: The number to convert to
/// * `decimals`: the number of decimals after the decimal point
/// # Returns
/// The number of bytes written if success or Error otherwise
pub fn fpi64_to_str(out: &mut [u8], value: i64, decimals: u8) -> Result<usize, ParserError> {
    let mut temp = [0u8; MAX_NUM_STR_BUFF_LEN];
    let len = i64_to_str(temp.as_mut(), value)?;
    fpstr_to_str(out, &temp[..len], decimals)
}

pub(crate) fn fpstr_to_str(
    out: &mut [u8],
    value: &[u8],
    decimals: u8,
) -> Result<usize, ParserError> {
    // zeroing memory
    for i in out.iter_mut() {
        *i = 0;
    }

    // Our buffer writer
    let mut writer = Writer::new(out);

    // Reproduce our input value as a str
    let str = core::str::from_utf8(value).map_err(|_| ParserError::parser_context_invalid_chars)?;
    let in_len = str.len();

    // edge case when no decimals
    // we should just copy whether zero
    // or the input value
    if decimals == 0 {
        if in_len == 0 {
            return write!(&mut writer, "{}", 0)
                .map(|_| 1)
                .map_err(|_| ParserError::parser_unexpected_buffer_end);
        }
        return write!(&mut writer, "{}", str)
            .map(|_| writer.offset)
            .map_err(|_| ParserError::parser_unexpected_buffer_end);
    }

    if in_len <= decimals as usize {
        if str.starts_with("-") {
            // we need to remove the sign before continuing
            let remainder = str
                .get(1..)
                .ok_or(ParserError::parser_unexpected_characters)?;
            return write!(&mut writer, "-0.{:0>1$}", remainder, decimals as usize)
                .map(|_| writer.offset)
                .map_err(|_| ParserError::parser_unexpected_buffer_end);
        }
        return write!(&mut writer, "0.{:0>1$}", str, decimals as usize)
            .map(|_| writer.offset)
            .map_err(|_| ParserError::parser_unexpected_buffer_end);
    }

    let fp = in_len - decimals as usize;
    let left = str.get(0..fp).unwrap();
    let right = str.get(fp..in_len).unwrap();
    write!(&mut writer, "{}.{}", left, right)
        .map(|_| writer.offset)
        .map_err(|_| ParserError::parser_unexpected_buffer_end)
}

pub fn pageString(out_value: &mut [u8], in_value: &[u8], page_idx: u8) -> Result<u8, ParserError> {
    // Just ensure the buffer is clear
    for i in out_value.iter_mut() {
        *i = 0u8;
    }
    let mut page_count;
    let out_len = out_value.len() - 1;
    let output = &mut out_value[..out_len];

    let in_len = in_value.len();

    if out_len == 0 || in_len == 0 {
        return Err(ParserError::parser_no_data);
    }
    page_count = (in_len / out_len) as u8;
    let last_chunk_len = in_len % out_len;

    if last_chunk_len > 0 {
        page_count += 1;
    }

    if page_idx < page_count {
        let idx = page_idx as usize * out_len;
        let last = if last_chunk_len > 0 && page_idx == page_count - 1 {
            idx + last_chunk_len
        } else {
            idx + out_len
        };
        let len = last - idx;
        (output[..len]).copy_from_slice(&in_value[idx..last]);
    }

    Ok(page_count)
}

#[cfg(test)]
mod test {
    extern crate std;
    use super::*;

    #[test]
    fn test_u64_to_str() {
        let mut output = [0u8; 10];
        assert!(u64_to_str(output.as_mut(), 125_550).is_ok());
        assert_eq!(&output[..6], b"125550");
        // overflow
        assert!(u64_to_str(output.as_mut(), 12521547982).is_err());
    }

    #[test]
    fn test_i64_to_str() {
        let mut output = [0u8; 10];
        assert!(i64_to_str(output.as_mut(), -125_550).is_ok());
        assert_eq!(&output[..7], b"-125550");
        // overflow
        assert!(i64_to_str(output.as_mut(), -1234567890).is_err());
    }

    #[test]
    fn test_fpi64_8decimals() {
        let mut output = [0u8; 15];
        let len = fpi64_to_str(output.as_mut(), -1234567, 8).unwrap();
        let result = core::str::from_utf8(&output[..len]).unwrap();
        assert_eq!(result, "-0.01234567");
    }

    #[test]
    fn test_fpi64_10decimals() {
        let mut output = [0u8; 15];
        // With 10 decimals
        let len = fpi64_to_str(output.as_mut(), -1234567, 10).unwrap();
        let result = core::str::from_utf8(&output[..len]).unwrap();
        assert_eq!(result, "-0.0001234567");
    }

    #[test]
    fn test_fpi64_0decimals() {
        let mut output = [0u8; 15];
        let len = fpi64_to_str(output.as_mut(), -1234567, 0).unwrap();
        let result = core::str::from_utf8(&output[..len]).unwrap();
        assert_eq!(result, "-1234567");
    }

    #[test]
    fn test_fpi64_4decimals() {
        let mut output = [0u8; 15];
        let len = fpi64_to_str(output.as_mut(), -1234567, 4).unwrap();
        let result = core::str::from_utf8(&output[..len]).unwrap();
        assert_eq!(result, "-123.4567");
    }

    #[test]
    fn test_fpi64_overflow() {
        let mut output = [0u8; 5];
        // overflow wit zero decimals
        let result = fpi64_to_str(output.as_mut(), -102123456, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_fpu64_8decimals() {
        let mut output = [0u8; 15];
        let len = fpu64_to_str(output.as_mut(), 1234567, 8).unwrap();
        let result = core::str::from_utf8(&output[..len]).unwrap();
        assert_eq!(result, "0.01234567");
    }

    #[test]
    fn test_fpu64_10decimals() {
        let mut output = [0u8; 15];
        // With 10 decimals
        let len = fpu64_to_str(output.as_mut(), 1234567, 10).unwrap();
        let result = core::str::from_utf8(&output[..len]).unwrap();
        assert_eq!(result, "0.0001234567");
    }

    #[test]
    fn test_fpu64_0decimals() {
        let mut output = [0u8; 15];
        // 0 decimals
        let len = fpu64_to_str(output.as_mut(), 1234567, 0).unwrap();
        let result = core::str::from_utf8(&output[..len]).unwrap();
        assert_eq!(result, "1234567");
    }

    #[test]
    fn test_fpu64_4decimals() {
        let mut output = [0u8; 15];
        let len = fpu64_to_str(output.as_mut(), 1234567, 4).unwrap();
        let result = core::str::from_utf8(&output[..len]).unwrap();
        assert_eq!(result, "123.4567");
    }

    #[test]
    fn test_fpu64_overflow() {
        let mut output = [0u8; 5];
        let result = fpu64_to_str(output.as_mut(), 1234567, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_paging_string() {
        let mut inValue = b"abcdabcdabcd";
        let mut outValue = [0u8; 6];
        let mut idx = 0;
        // the pageString will left over the last byte
        // as a string terminator, so we make chunks of outValue.len() - 1
        for chunk in inValue.chunks(outValue.len() - 1) {
            pageString(outValue.as_mut(), inValue.as_ref(), idx).unwrap();
            assert_eq!(outValue[..chunk.len()].as_ref(), chunk);
            idx += 1;
        }
    }
}
