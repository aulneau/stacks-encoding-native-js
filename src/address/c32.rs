// Copyright (C) 2013-2020 Blockstack PBC, a public benefit corporation
// Copyright (C) 2020 Stacks Open Internet Foundation
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use super::Error;

use sha2::Digest;
use sha2::Sha256;
use std::convert::TryFrom;

const C32_CHARACTERS: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// C32 chars as an array, indexed by their ASCII code for O(1) lookups.
/// Supports lookups by uppercase, lowercase, and special (i.e. `O, L, I`) chars.
/// Table can be generated with:
/// ```
/// let mut table: [isize; 128] = [-1; 128];
/// let alphabet = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// for (i, x) in alphabet.as_bytes().iter().enumerate() {
///     table[*x as usize] = i as isize;
/// }
/// let alphabet_lower = alphabet.to_lowercase();
/// for (i, x) in alphabet_lower.as_bytes().iter().enumerate() {
///     table[*x as usize] = i as isize;
/// }
/// let specials = [('O', '0'), ('L', '1'), ('I', '1')];
/// for pair in specials {
///     let i = alphabet.find(|a| a == pair.1).unwrap() as isize;
///     table[pair.0 as usize] = i;
///     table[pair.0.to_ascii_lowercase() as usize] = i;
/// }
/// ```
const C32_CHARACTERS_MAP: [i8; 128] = [
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, -1, -1, -1, -1, -1, -1, -1, 10, 11, 12, 13, 14, 15, 16, 17, 1,
    18, 19, 1, 20, 21, 0, 22, 23, 24, 25, 26, -1, 27, 28, 29, 30, 31, -1, -1, -1, -1, -1, -1, 10,
    11, 12, 13, 14, 15, 16, 17, 1, 18, 19, 1, 20, 21, 0, 22, 23, 24, 25, 26, -1, 27, 28, 29, 30,
    31, -1, -1, -1, -1, -1,
];

fn c32_encode(input_bytes: &[u8]) -> String {
    let capacity = get_max_c32_encode_output_len(input_bytes.len());
    let mut buffer: Vec<u8> = vec![0; capacity];
    let bytes_written = c32_encode_to_buffer(input_bytes, &mut buffer).unwrap();
    buffer.truncate(bytes_written);
    String::from_utf8(buffer).unwrap()
}

/// Calculate the maximum C32 encoded output size given an input size.
/// Each C32 character encodes 5 bits.
fn get_max_c32_encode_output_len(input_len: usize) -> usize {
    let capacity = (input_len as f64 + (input_len % 5) as f64) / 5.0 * 8.0;
    capacity as usize
}

/// C32 encodes input bytes into an output buffer. Returns the number of bytes written to the
/// output buffer.
/// # Arguments
/// * `output_buffer` - A mutable slice where the C32 encoded bytes are written. An error
/// result is returned if the length is smaller than the maximum possible output length. Each
/// C32 character encodes 5 bits; use `get_max_c32_encode_output_len` to easily determine the
/// minimum length.
///
/// # Examples
///
/// ```no_run
/// let input_bytes = b"hello world";
/// let capacity = get_max_c32_encode_output_len(input_bytes.len());
/// let mut buffer: Vec<u8> = vec![0; capacity];
/// let bytes_written = c32_encode_to_buffer(input_bytes, &mut buffer).unwrap();
/// buffer.truncate(bytes_written);
/// String::from_utf8(buffer)
/// ```
fn c32_encode_to_buffer(input_bytes: &[u8], output_buffer: &mut [u8]) -> Result<usize, Error> {
    let min_len = get_max_c32_encode_output_len(input_bytes.len());
    if output_buffer.len() < min_len {
        Err(Error::Other(format!(
            "C32 encode output buffer is too small, given size {}, need minimum size {}",
            output_buffer.len(),
            min_len
        )))?
    }
    let mut carry = 0;
    let mut carry_bits = 0;
    let mut position = 0;

    for current_value in input_bytes.iter().rev() {
        let low_bits_to_take = 5 - carry_bits;
        let low_bits = current_value & ((1 << low_bits_to_take) - 1);
        let c32_value = (low_bits << carry_bits) + carry;

        output_buffer[position] = C32_CHARACTERS[c32_value as usize];
        position += 1;

        carry_bits = (8 + carry_bits) - 5;
        carry = current_value >> (8 - carry_bits);

        if carry_bits >= 5 {
            let c32_value = carry & ((1 << 5) - 1);

            output_buffer[position] = C32_CHARACTERS[c32_value as usize];
            position += 1;

            carry_bits = carry_bits - 5;
            carry = carry >> 5;
        }
    }

    if carry_bits > 0 {
        output_buffer[position] = C32_CHARACTERS[carry as usize];
        position += 1;
    }

    // remove leading zeros from c32 encoding
    while position > 0 && output_buffer[position - 1] == C32_CHARACTERS[0] {
        position -= 1;
    }

    // add leading zeros from input.
    for current_value in input_bytes.iter() {
        if *current_value == 0 {
            output_buffer[position] = C32_CHARACTERS[0];
            position += 1;
        } else {
            break;
        }
    }

    output_buffer[..position].reverse();
    Ok(position)
}

fn c32_decode(input_str: &str) -> Result<Vec<u8>, Error> {
    // must be ASCII
    if !input_str.is_ascii() {
        return Err(Error::InvalidCrockford32);
    }
    c32_decode_ascii(input_str.as_bytes())
}

fn c32_decode_ascii(input_str: &[u8]) -> Result<Vec<u8>, Error> {
    // let initial_capacity = 1 + ((input_str.len() * 5) / 8);
    let initial_capacity = input_str.len();
    let mut result = Vec::with_capacity(initial_capacity);
    let mut carry: u16 = 0;
    let mut carry_bits = 0; // can be up to 5

    let mut c32_digits = vec![0u8; input_str.len()];

    for (i, x) in input_str.iter().rev().enumerate() {
        c32_digits[i] = match C32_CHARACTERS_MAP.get(*x as usize) {
            Some(v) => match u8::try_from(*v) {
                Ok(v) => Ok(v),
                Err(_) => Err(Error::InvalidCrockford32),
            },
            None => Err(Error::InvalidCrockford32),
        }?;
    }

    for current_5bit in &c32_digits {
        carry += (*current_5bit as u16) << carry_bits;
        carry_bits += 5;

        if carry_bits >= 8 {
            result.push((carry & ((1 << 8) - 1)) as u8);
            carry_bits -= 8;
            carry = carry >> 8;
        }
    }

    if carry_bits > 0 {
        result.push(carry as u8);
    }

    // remove leading zeros from Vec<u8> encoding
    let mut i = result.len();
    while i > 0 && result[i - 1] == 0 {
        i -= 1;
        result.truncate(i);
    }

    // add leading zeros from input.
    for current_value in c32_digits.iter().rev() {
        if *current_value == 0 {
            result.push(0);
        } else {
            break;
        }
    }

    result.reverse();
    Ok(result)
}

fn c32_check_encode_prefixed(version: u8, data: &[u8], prefix: u8) -> Result<String, Error> {
    if version >= 32 {
        return Err(Error::InvalidVersion(version));
    }

    let data_len = data.len();
    let mut buffer: Vec<u8> = vec![0; data_len + 4];

    let checksum_buffer = Sha256::digest({
        Sha256::new()
            .chain_update(&[version])
            .chain_update(data)
            .finalize()
    });

    buffer[..data_len].copy_from_slice(data);
    buffer[data_len..(data_len + 4)].copy_from_slice(&checksum_buffer[0..4]);

    let capacity = get_max_c32_encode_output_len(buffer.len()) + 2;
    let mut result: Vec<u8> = vec![0; capacity];

    result[0] = prefix;
    result[1] = C32_CHARACTERS[version as usize];
    let bytes_written = c32_encode_to_buffer(&buffer, &mut result[2..])?;
    result.truncate(bytes_written + 2);
    Ok(String::from_utf8(result).unwrap())
}

fn c32_check_decode(check_data_unsanitized: &str) -> Result<(u8, Vec<u8>), Error> {
    // must be ASCII
    if !check_data_unsanitized.is_ascii() {
        return Err(Error::InvalidCrockford32);
    }

    if check_data_unsanitized.len() < 2 {
        return Err(Error::InvalidCrockford32);
    }

    let ascii_bytes = check_data_unsanitized.as_bytes();
    let (version, data) = ascii_bytes.split_first().unwrap();

    let data_sum_bytes = c32_decode_ascii(data)?;
    if data_sum_bytes.len() < 4 {
        return Err(Error::InvalidCrockford32);
    }

    let (data_bytes, expected_sum) = data_sum_bytes.split_at(data_sum_bytes.len() - 4);
    let decoded_version = c32_decode_ascii(&[*version]).unwrap();
    let computed_sum = Sha256::digest(
        Sha256::new()
            .chain_update(&decoded_version)
            .chain_update(&data_bytes)
            .finalize(),
    );
    let checksum_ok = {
        computed_sum[0] == expected_sum[0]
            && computed_sum[1] == expected_sum[1]
            && computed_sum[2] == expected_sum[2]
            && computed_sum[3] == expected_sum[3]
    };
    if !checksum_ok {
        let computed_sum_u32 = (computed_sum[0] as u32)
            | ((computed_sum[1] as u32) << 8)
            | ((computed_sum[2] as u32) << 16)
            | ((computed_sum[3] as u32) << 24);

        let expected_sum_u32 = (expected_sum[0] as u32)
            | ((expected_sum[1] as u32) << 8)
            | ((expected_sum[2] as u32) << 16)
            | ((expected_sum[3] as u32) << 24);

        return Err(Error::BadChecksum(computed_sum_u32, expected_sum_u32));
    }

    let version = decoded_version[0];
    let data = data_bytes.to_vec();
    Ok((version, data))
}

pub fn c32_address_decode(c32_address_str: &str) -> Result<(u8, Vec<u8>), Error> {
    if c32_address_str.len() <= 5 {
        Err(Error::InvalidCrockford32)
    } else {
        c32_check_decode(&c32_address_str[1..])
    }
}

pub fn c32_address(version: u8, data: &[u8]) -> Result<String, Error> {
    c32_check_encode_prefixed(version, data, b'S')
}

#[cfg(test)]
mod test {
    use crate::hex::decode_hex;

    use super::*;

    #[test]
    fn test_addresses() {
        let hex_strs = [
            "a46ff88886c2ef9762d970b4d2c63678835bd39d",
            "0000000000000000000000000000000000000000",
            "0000000000000000000000000000000000000001",
            "1000000000000000000000000000000000000001",
            "1000000000000000000000000000000000000000",
        ];

        let versions = [22, 0, 31, 20, 26, 21];

        let c32_addrs = [
            [
                "SP2J6ZY48GV1EZ5V2V5RB9MP66SW86PYKKNRV9EJ7",
                "SP000000000000000000002Q6VF78",
                "SP00000000000000000005JA84HQ",
                "SP80000000000000000000000000000004R0CMNV",
                "SP800000000000000000000000000000033H8YKK",
            ],
            [
                "S02J6ZY48GV1EZ5V2V5RB9MP66SW86PYKKPVKG2CE",
                "S0000000000000000000002AA028H",
                "S000000000000000000006EKBDDS",
                "S080000000000000000000000000000007R1QC00",
                "S080000000000000000000000000000003ENTGCQ",
            ],
            [
                "SZ2J6ZY48GV1EZ5V2V5RB9MP66SW86PYKKQ9H6DPR",
                "SZ000000000000000000002ZE1VMN",
                "SZ00000000000000000005HZ3DVN",
                "SZ80000000000000000000000000000004XBV6MS",
                "SZ800000000000000000000000000000007VF5G0",
            ],
            [
                "SM2J6ZY48GV1EZ5V2V5RB9MP66SW86PYKKQVX8X0G",
                "SM0000000000000000000062QV6X",
                "SM00000000000000000005VR75B2",
                "SM80000000000000000000000000000004WBEWKC",
                "SM80000000000000000000000000000000JGSYGV",
            ],
            [
                "ST2J6ZY48GV1EZ5V2V5RB9MP66SW86PYKKQYAC0RQ",
                "ST000000000000000000002AMW42H",
                "ST000000000000000000042DB08Y",
                "ST80000000000000000000000000000006BYJ4R4",
                "ST80000000000000000000000000000002YBNPV3",
            ],
            [
                "SN2J6ZY48GV1EZ5V2V5RB9MP66SW86PYKKP6D2ZK9",
                "SN000000000000000000003YDHWKJ",
                "SN00000000000000000005341MC8",
                "SN800000000000000000000000000000066KZWY0",
                "SN800000000000000000000000000000006H75AK",
            ],
        ];

        for i in 0..hex_strs.len() {
            for j in 0..versions.len() {
                let h = hex_strs[i];
                let v = versions[j];
                let b = decode_hex(h).unwrap();
                let z = c32_address(v, &b).unwrap();

                assert_eq!(z, c32_addrs[j][i]);

                let (decoded_version, decoded_bytes) = c32_address_decode(&z).unwrap();
                assert_eq!(decoded_version, v);
                assert_eq!(decoded_bytes.as_slice(), b.as_ref());
            }
        }
    }

    #[test]
    fn test_simple() {
        let hex_strings = &[
            "a46ff88886c2ef9762d970b4d2c63678835bd39d",
            "",
            "0000000000000000000000000000000000000000",
            "0000000000000000000000000000000000000001",
            "1000000000000000000000000000000000000001",
            "1000000000000000000000000000000000000000",
            "01",
            "22",
            "0001",
            "000001",
            "00000001",
            "10",
            "0100",
            "1000",
            "010000",
            "100000",
            "01000000",
            "10000000",
            "0100000000",
        ];
        let c32_strs = [
            "MHQZH246RBQSERPSE2TD5HHPF21NQMWX",
            "",
            "00000000000000000000",
            "00000000000000000001",
            "20000000000000000000000000000001",
            "20000000000000000000000000000000",
            "1",
            "12",
            "01",
            "001",
            "0001",
            "G",
            "80",
            "400",
            "2000",
            "10000",
            "G0000",
            "800000",
            "4000000",
        ];

        let results: Vec<_> = hex_strings
            .iter()
            .zip(c32_strs.iter())
            .map(|(hex_str, expected)| {
                let bytes = decode_hex(hex_str).unwrap();
                let c32_encoded = c32_encode(&bytes);
                let decoded_bytes = c32_decode(&c32_encoded).unwrap();
                let result = (bytes, c32_encoded, decoded_bytes, expected);
                println!("{:?}", result);
                result
            })
            .collect();
        for (bytes, c32_encoded, decoded_bytes, expected_c32) in results.iter() {
            assert_eq!(bytes.as_ref(), decoded_bytes);
            assert_eq!(c32_encoded, *expected_c32);
        }
    }

    #[test]
    fn test_normalize() {
        let addrs = [
            "S02J6ZY48GV1EZ5V2V5RB9MP66SW86PYKKPVKG2CE",
            "SO2J6ZY48GV1EZ5V2V5RB9MP66SW86PYKKPVKG2CE",
            "S02J6ZY48GVLEZ5V2V5RB9MP66SW86PYKKPVKG2CE",
            "SO2J6ZY48GVLEZ5V2V5RB9MP66SW86PYKKPVKG2CE",
            "s02j6zy48gv1ez5v2v5rb9mp66sw86pykkpvkg2ce",
            "sO2j6zy48gv1ez5v2v5rb9mp66sw86pykkpvkg2ce",
            "s02j6zy48gvlez5v2v5rb9mp66sw86pykkpvkg2ce",
            "sO2j6zy48gvlez5v2v5rb9mp66sw86pykkpvkg2ce",
        ];

        let expected_bytes = decode_hex("a46ff88886c2ef9762d970b4d2c63678835bd39d").unwrap();
        let expected_version = 0;

        for addr in addrs.iter() {
            let (decoded_version, decoded_bytes) = c32_address_decode(addr).unwrap();
            assert_eq!(decoded_version, expected_version);
            assert_eq!(decoded_bytes, expected_bytes.as_ref());
        }
    }

    #[test]
    fn test_ascii_only() {
        match c32_address_decode("S\u{1D7D8}2J6ZY48GV1EZ5V2V5RB9MP66SW86PYKKPVKG2CE") {
            Err(Error::InvalidCrockford32) => {}
            _ => {
                assert!(false);
            }
        }
    }
}
