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

use crate::num_bits;
use std::collections::HashMap;

/// Histogram with logarithmic buckets.
#[derive(Default)]
pub struct Histogram {
    /// Maps each bucket to its count.
    ///
    /// Buckets are numbers sharing the same highest bit. The first buckets are: only 0, only 1, 2
    /// to 3, 4 to 7, 8 to 15. Buckets are identified by their lower-bound.
    buckets: HashMap<usize, usize>,
}

impl Histogram {
    /// Increases the count of the bucket of an item.
    ///
    /// The bucket of `item` is the highest power of two, lower or equal to `item`. If `item` is
    /// zero, then its bucket is also zero.
    ///
    /// # Panics
    ///
    /// Panics if the item is too big, i.e. it uses its most significant bit.
    pub fn add(&mut self, item: usize) {
        assert!(item <= usize::max_value() / 2);
        *self.buckets.entry(get_bucket(item)).or_insert(0) += 1;
    }

    /// Merges another histogram into this one.
    pub fn merge(&mut self, other: &Histogram) {
        for (&bucket, &count) in &other.buckets {
            *self.buckets.entry(bucket).or_insert(0) += count;
        }
    }

    /// Returns one past the highest non-empty bucket.
    ///
    /// In other words, all non-empty buckets of the histogram are smaller than the returned bucket.
    pub fn bucket_lim(&self) -> usize {
        match self.buckets.keys().max() {
            None => 0,
            Some(0) => 1,
            Some(x) => 2 * x,
        }
    }

    /// Returns the count of a bucket.
    pub fn get(&self, bucket: usize) -> Option<usize> {
        self.buckets.get(&bucket).cloned()
    }

    /// Returns the total count.
    pub fn count(&self) -> usize {
        self.buckets.values().sum()
    }
}

/// Returns the bucket of an item.
fn get_bucket(item: usize) -> usize {
    let bucket = bucket_from_width(num_bits(item));
    assert!(bucket <= item && (item == 0 || item / 2 < bucket));
    bucket
}

/// Returns the bucket of an item given its bit-width.
pub fn bucket_from_width(width: usize) -> usize {
    if width == 0 {
        0
    } else {
        1 << (width - 1)
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
