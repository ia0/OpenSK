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

use std::collections::HashMap;

/// Histogram with logarithmic buckets
#[derive(Default)]
pub struct Histogram {
    buckets: HashMap<usize, usize>,
}

impl Histogram {
    /// Increases the count of the bucket of `item`
    ///
    /// The bucket of `item` is the highest power of two, lower or equal to `item`. If `item` is
    /// zero, then its bucket is also zero.
    pub fn add(&mut self, item: usize) {
        *self.buckets.entry(get_bucket(item)).or_insert(0) += 1;
    }

    pub fn merge(&mut self, other: &Histogram) {
        for (&bucket, &count) in &other.buckets {
            *self.buckets.entry(bucket).or_insert(0) += count;
        }
    }

    /// Returns one past the highest non-empty bucket
    ///
    /// In other words, all non-empty buckets of the histogram are smaller than the returned bucket.
    pub fn bucket_lim(&self) -> usize {
        match self.buckets.keys().max() {
            None => 0,
            Some(x) => 2 * x,
        }
    }

    pub fn get(&self, bucket: usize) -> Option<usize> {
        self.buckets.get(&bucket).cloned()
    }

    pub fn count(&self) -> usize {
        self.buckets.values().sum()
    }
}

fn get_bucket(item: usize) -> usize {
    let width = 8 * std::mem::size_of::<usize>() - item.leading_zeros() as usize;
    let bucket = bucket_from_width(width);
    assert!(bucket <= item && (item == 0 || item / 2 < bucket));
    bucket
}

/// Converts a number of bits into a bucket
///
/// The number of bits should be the minimum number of bits necessary to represent the item.
pub fn bucket_from_width(width: usize) -> usize {
    if width == 0 {
        0
    } else {
        1 << width - 1
    }
}

#[test]
fn get_bucket_ok() {
    assert_eq!(get_bucket(0), 0);
    assert_eq!(get_bucket(1), 1);
    assert_eq!(get_bucket(2), 2);
    assert_eq!(get_bucket(3), 2);
    assert_eq!(get_bucket(4), 4);
    assert_eq!(get_bucket(7), 4);
    assert_eq!(get_bucket(8), 8);
    assert_eq!(get_bucket(15), 8);
}
