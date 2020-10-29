// Copyright 2019-2020 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//      http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

pub mod histogram;
pub mod stats;
pub mod store;

pub struct Entropy<'a> {
    data: &'a [u8],
    bit: usize,
}

const MAX: usize = usize::max_value() / 2;

impl Entropy<'_> {
    pub fn new(data: &[u8]) -> Entropy {
        let bit = 0;
        Entropy { data, bit }
    }

    pub fn consume_all(&mut self) {
        self.bit = 8 * self.data.len();
    }

    pub fn is_empty(&self) -> bool {
        assert!(self.bit <= 8 * self.data.len());
        self.bit == 8 * self.data.len()
    }

    /// Reads a bit.
    pub fn read_bit(&mut self) -> bool {
        if self.is_empty() {
            return false;
        }
        let b = self.bit;
        self.bit += 1;
        self.data[b / 8] & 1 << b % 8 != 0
    }

    /// Reads `n` bits.
    pub fn read_bits(&mut self, n: usize) -> usize {
        assert!(n <= 8 * std::mem::size_of::<usize>());
        let mut r = 0;
        for i in 0..n {
            r |= (self.read_bit() as usize) << i;
        }
        r
    }

    /// Reads a byte.
    pub fn read_byte(&mut self) -> u8 {
        self.read_bits(8) as u8
    }

    /// Reads a slice.
    pub fn read_slice(&mut self, length: usize) -> Vec<u8> {
        let mut result = Vec::with_capacity(length);
        for _ in 0..length {
            result.push(self.read_byte());
        }
        result
    }

    /// Reads a number between `min` and `max`.
    pub fn read_range(&mut self, min: usize, max: usize) -> usize {
        assert!(min <= max && max <= MAX);
        let width = max - min;
        let delta = self.read_bits(num_bits(width)) % (width + 1);
        min + delta
    }

    /// Reads a possibly invalid number between `min` and `max`.
    ///
    /// In addition to valid values between `min` and `max`, a call may also return any of the
    /// following invalid values: `max + 1` and `u32::MAX`.
    pub fn read_range_overflow(&mut self, min: usize, max: usize) -> usize {
        let value = self.read_range(min, max + 2);
        if value > 0 {
            value - 1
        } else {
            u32::MAX as usize
        }
    }
}

/// Returns the number of bits necessary to represent `x`.
fn num_bits(x: usize) -> usize {
    8 * core::mem::size_of::<usize>() - x.leading_zeros() as usize
}

#[test]
fn num_bits_ok() {
    assert_eq!(num_bits(0), 0);
    assert_eq!(num_bits(1), 1);
    assert_eq!(num_bits(2), 2);
    assert_eq!(num_bits(3), 2);
    assert_eq!(num_bits(4), 3);
    assert_eq!(num_bits(7), 3);
    assert_eq!(num_bits(8), 4);
    assert_eq!(num_bits(15), 4);
    assert_eq!(num_bits(16), 5);
}

#[test]
fn read_bit_ok() {
    let mut entropy = Entropy::new(&[0b10110010]);
    assert!(!entropy.read_bit());
    assert!(entropy.read_bit());
    assert!(!entropy.read_bit());
    assert!(!entropy.read_bit());
    assert!(entropy.read_bit());
    assert!(entropy.read_bit());
    assert!(!entropy.read_bit());
    assert!(entropy.read_bit());
}

#[test]
fn read_bits_ok() {
    let mut entropy = Entropy::new(&[0x83, 0x92]);
    assert_eq!(entropy.read_bits(4), 0x3);
    assert_eq!(entropy.read_bits(8), 0x28);
    assert_eq!(entropy.read_bits(2), 1);
    assert_eq!(entropy.read_bits(2), 2);
}

#[test]
fn read_range_ok() {
    let mut entropy = Entropy::new(&[0x2b]);
    assert_eq!(entropy.read_range(0, 7), 3);
    assert_eq!(entropy.read_range(1, 8), 6);
    assert_eq!(entropy.read_range(4, 6), 4);
    let mut entropy = Entropy::new(&[0x2b]);
    assert_eq!(entropy.read_range(0, 8), 2);
    assert_eq!(entropy.read_range(3, 15), 5);
}
