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

use crate::histogram::{bucket_from_width, Histogram};

use std::collections::HashMap;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum StatKey {
    Entropy,
    PageSize,
    NumPages,
    MaxPageErases,
    DirtyLength,
    InitCycles,
    Lifetime,
    ReachedLifetime,
    Compaction,
    PowerOnCount,
    TransactionCount,
    ClearCount,
    PrepareCount,
    InsertCount,
    RemoveCount,
    InterruptionCount,
}

pub const ALL_KEYS: &[StatKey] = &[
    StatKey::Entropy,
    StatKey::PageSize,
    StatKey::NumPages,
    StatKey::MaxPageErases,
    StatKey::DirtyLength,
    StatKey::InitCycles,
    StatKey::Lifetime,
    StatKey::ReachedLifetime,
    StatKey::Compaction,
    StatKey::PowerOnCount,
    StatKey::TransactionCount,
    StatKey::ClearCount,
    StatKey::PrepareCount,
    StatKey::InsertCount,
    StatKey::RemoveCount,
    StatKey::InterruptionCount,
];

pub const ALL_COUNTERS: &[StatKey] = &[
    StatKey::PowerOnCount,
    StatKey::TransactionCount,
    StatKey::ClearCount,
    StatKey::PrepareCount,
    StatKey::InsertCount,
    StatKey::RemoveCount,
    StatKey::InterruptionCount,
];

impl StatKey {
    pub fn name(self) -> &'static str {
        use StatKey::*;
        match self {
            Entropy => "Entropy",
            PageSize => "Page size",
            NumPages => "Num page",
            MaxPageErases => "Max erase cycle",
            DirtyLength => "Dirty length",
            InitCycles => "Initial cycles",
            Lifetime => "Used lifetime",
            ReachedLifetime => "Reached lifetime",
            Compaction => "Num compaction",
            PowerOnCount => "Num power on",
            TransactionCount => "Num transaction",
            ClearCount => "Num clear",
            PrepareCount => "Num prepare",
            InsertCount => "Num insert",
            RemoveCount => "Num remove",
            InterruptionCount => "Num interruption",
        }
    }
}

#[derive(Default)]
pub struct Stats {
    stats: HashMap<StatKey, Histogram>,
}

impl Stats {
    pub fn add(&mut self, key: StatKey, value: usize) {
        self.stats.entry(key).or_default().add(value);
    }

    pub fn merge(&mut self, other: &Stats) {
        for (&key, other) in &other.stats {
            self.stats.entry(key).or_default().merge(other);
        }
    }

    /// Returns one past the highest non-empty bucket
    ///
    /// In other words, all non-empty buckets of the histogram are smaller than the returned bucket.
    fn bucket_lim(&self) -> usize {
        self.stats
            .values()
            .map(|h| h.bucket_lim())
            .max()
            .unwrap_or(0)
    }
}

impl std::fmt::Display for Stats {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        let mut matrix: Vec<Vec<String>> = Vec::new();

        let mut header = Vec::new();
        header.push(String::new());
        let bucket_lim = self.bucket_lim();
        let bits = bucket_lim.trailing_zeros() as usize;
        for width in 0..=bits {
            let bucket = bucket_from_width(width);
            header.push(format!(" {}", bucket));
        }
        header.push(" count".into());
        matrix.push(header);

        for &key in ALL_KEYS {
            let mut row = Vec::new();
            row.push(format!("{}:", key.name()));
            if let Some(h) = self.stats.get(&key) {
                for width in 0..=bits {
                    let bucket = bucket_from_width(width);
                    row.push(match h.get(bucket) {
                        None => String::new(),
                        Some(x) => format!(" {}", x),
                    });
                }
                row.push(format!(" {}", h.count()));
            }
            matrix.push(row);
        }

        write_matrix(f, matrix)
    }
}

fn align(f: &mut std::fmt::Formatter, x: &str, n: usize) -> Result<(), std::fmt::Error> {
    for _ in 0..n.saturating_sub(x.len()) {
        write!(f, " ")?;
    }
    write!(f, "{}", x)
}

fn write_matrix(
    f: &mut std::fmt::Formatter,
    mut m: Vec<Vec<String>>,
) -> Result<(), std::fmt::Error> {
    if m.len() == 0 {
        return Ok(());
    }
    let num_cols = m.iter().map(|r| r.len()).max().unwrap();
    let mut col_len = vec![0; num_cols];
    for row in &mut m {
        row.resize(num_cols, String::new());
        for col in 0..num_cols {
            col_len[col] = std::cmp::max(col_len[col], row[col].len());
        }
    }
    for row in m {
        for col in 0..num_cols {
            align(f, &row[col], col_len[col])?;
        }
        writeln!(f)?;
    }
    Ok(())
}
