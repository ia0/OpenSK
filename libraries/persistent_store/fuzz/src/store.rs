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

use crate::stats::{StatKey, Stats, ALL_COUNTERS};
use crate::Entropy;
use persistent_store::{
    BufferOptions, BufferStorage, Store, StoreDriver, StoreDriverOff, StoreDriverOn,
    StoreInterruption, StoreInvariant, StoreOperation, StoreUpdate,
};
use rand_core::{RngCore, SeedableRng};
use rand_pcg::Pcg32;
use std::collections::HashMap;
use std::convert::TryInto;

pub fn fuzz(data: &[u8], debug: bool, stats: Option<&mut Stats>) {
    let mut fuzzer = Fuzzer::new(data, debug, stats);
    fuzzer.init_counters();
    fuzzer.record(StatKey::Entropy, data.len());
    let mut driver = fuzzer.init();
    let store = loop {
        if fuzzer.debug {
            print!("{}", driver.storage());
        }
        if let StoreDriver::On(driver) = &driver {
            if !fuzzer.init.is_dirty() {
                driver.check().unwrap();
            }
            if fuzzer.debug {
                println!("----------------------------------------------------------------------");
            }
        }
        if fuzzer.entropy.is_empty() {
            if fuzzer.debug {
                println!("No more entropy.");
            }
            if fuzzer.init.is_dirty() {
                return;
            }
            fuzzer.record(StatKey::ReachedLifetime, 0);
            break driver.power_on().unwrap().into_store();
        }
        driver = match driver {
            StoreDriver::On(driver) => match fuzzer.apply(driver) {
                Ok(x) => x,
                Err(store) => {
                    if fuzzer.debug {
                        println!("No more lifetime.");
                    }
                    if fuzzer.init.is_dirty() {
                        return;
                    }
                    fuzzer.record(StatKey::ReachedLifetime, 1);
                    break store;
                }
            },
            StoreDriver::Off(driver) => fuzzer.power_on(driver),
        }
    };
    let virt_window = store.format().num_pages() * store.format().virt_page_size();
    let init_lifetime = fuzzer.init.used_cycles() * virt_window;
    let lifetime = store.lifetime().unwrap().used - init_lifetime;
    fuzzer.record(StatKey::Lifetime, lifetime);
    fuzzer.record(StatKey::Compaction, lifetime / virt_window);
    fuzzer.record_counters();
}

struct Fuzzer<'a> {
    entropy: Entropy<'a>,
    values: Pcg32,
    init: Init,
    debug: bool,
    stats: Option<&'a mut Stats>,
    counters: HashMap<StatKey, usize>,
}

impl<'a> Fuzzer<'a> {
    fn new(data: &'a [u8], debug: bool, stats: Option<&'a mut Stats>) -> Fuzzer<'a> {
        let mut entropy = Entropy::new(data);
        let seed = entropy.read_slice(16);
        let values = Pcg32::from_seed(seed[..].try_into().unwrap());
        Fuzzer {
            entropy,
            values,
            init: Init::Clean,
            debug,
            stats,
            counters: HashMap::new(),
        }
    }

    fn init(&mut self) -> StoreDriver {
        let mut options = BufferOptions {
            word_size: 4,
            page_size: 1 << self.entropy.read_range(5, 12),
            max_word_writes: 2,
            max_page_erases: self.entropy.read_range(0, 50000),
            strict_write: true,
        };
        let num_pages = self.entropy.read_range(3, 64);
        self.record(StatKey::PageSize, options.page_size);
        self.record(StatKey::MaxPageErases, options.max_page_erases);
        self.record(StatKey::NumPages, num_pages);
        if self.debug {
            println!("page_size: {}", options.page_size);
            println!("num_pages: {}", num_pages);
            println!("max_cycle: {}", options.max_page_erases);
        }
        let storage_size = num_pages * options.page_size;
        if self.entropy.read_bit() {
            self.init = Init::Dirty;
            let mut storage = vec![0xff; storage_size].into_boxed_slice();
            let length = self.entropy.read_range(0, storage_size);
            self.record(StatKey::DirtyLength, length);
            for byte in &mut storage[0..length] {
                *byte = self.entropy.read_byte();
            }
            if self.debug {
                println!("Start with dirty storage.");
            }
            options.strict_write = false;
            let storage = BufferStorage::new(storage, options);
            StoreDriver::Off(StoreDriverOff::new_dirty(storage))
        } else if self.entropy.read_bit() {
            let cycle = self.entropy.read_range(0, options.max_page_erases);
            self.init = Init::Used { cycle };
            if self.debug {
                println!("Start with {} consumed erase cycles.", cycle);
            }
            self.record(StatKey::InitCycles, cycle);
            let storage = vec![0xff; storage_size].into_boxed_slice();
            let mut storage = BufferStorage::new(storage, options);
            Store::init_with_cycle(&mut storage, cycle);
            StoreDriver::Off(StoreDriverOff::new_dirty(storage))
        } else {
            StoreDriver::Off(StoreDriverOff::new(options, num_pages))
        }
    }

    fn power_on(&mut self, driver: StoreDriverOff) -> StoreDriver {
        if self.debug {
            println!("Power on the store.");
        }
        self.increment(StatKey::PowerOnCount);
        let interruption = self.interruption(driver.delay_map());
        match driver.partial_power_on(interruption) {
            Err((storage, _)) if self.init.is_dirty() => {
                self.entropy.consume_all();
                StoreDriver::Off(StoreDriverOff::new_dirty(storage))
            }
            Err(error) => self.crash(error),
            Ok(driver) => driver,
        }
    }

    fn apply(&mut self, driver: StoreDriverOn) -> Result<StoreDriver, Store<BufferStorage>> {
        let operation = self.operation(&driver);
        if self.debug {
            println!("{:?}", operation);
        }
        let interruption = self.interruption(driver.delay_map(&operation));
        match driver.partial_apply(operation, interruption) {
            Err((store, _)) if self.init.is_dirty() => {
                self.entropy.consume_all();
                Err(store)
            }
            Err((store, StoreInvariant::NoLifetime)) => Err(store),
            Err((store, error)) => self.crash((store.into_storage(), error)),
            Ok((error, driver)) => {
                if self.debug {
                    if let Some(error) = error {
                        println!("{:?}", error);
                    }
                }
                Ok(driver)
            }
        }
    }

    fn crash(&self, error: (BufferStorage, StoreInvariant)) -> ! {
        let (storage, invariant) = error;
        if self.debug {
            print!("{}", storage);
        }
        panic!("{:?}", invariant);
    }

    fn record(&mut self, key: StatKey, value: usize) {
        if let Some(stats) = &mut self.stats {
            stats.add(key, value);
        }
    }

    fn increment(&mut self, key: StatKey) {
        if self.stats.is_some() {
            *self.counters.get_mut(&key).unwrap() += 1;
        }
    }

    fn init_counters(&mut self) {
        if self.stats.is_some() {
            for &key in ALL_COUNTERS {
                self.counters.insert(key, 0);
            }
        }
    }

    fn record_counters(&mut self) {
        if let Some(stats) = &mut self.stats {
            for (&key, &value) in self.counters.iter() {
                stats.add(key, value);
            }
        }
    }

    fn operation(&mut self, driver: &StoreDriverOn) -> StoreOperation {
        let format = driver.model().format();
        match self.entropy.read_range(0, 2) {
            0 => {
                // Use one past as the canonical invalid number of updates.
                let count = self.entropy.read_range(0, format.max_updates() + 1);
                let mut updates = Vec::with_capacity(count);
                for _ in 0..count {
                    updates.push(self.update());
                }
                self.increment(StatKey::TransactionCount);
                StoreOperation::Transaction { updates }
            }
            1 => {
                let min_key = self.key();
                self.increment(StatKey::ClearCount);
                StoreOperation::Clear { min_key }
            }
            2 => {
                // Use one past as the canonical invalid required capacity.
                let length = self.entropy.read_range(0, format.total_capacity() + 1);
                self.increment(StatKey::PrepareCount);
                StoreOperation::Prepare { length }
            }
            _ => unreachable!(),
        }
    }

    fn update(&mut self) -> StoreUpdate {
        match self.entropy.read_range(0, 1) {
            0 => {
                let key = self.key();
                let value = self.value();
                self.increment(StatKey::InsertCount);
                StoreUpdate::Insert { key, value }
            }
            1 => {
                let key = self.key();
                self.increment(StatKey::RemoveCount);
                StoreUpdate::Remove { key }
            }
            _ => unreachable!(),
        }
    }

    fn key(&mut self) -> usize {
        // Use 4096 as the canonical invalid key.
        self.entropy.read_range(0, 4096)
    }

    fn value(&mut self) -> Vec<u8> {
        // Use 1024 as the canonical invalid length.
        let length = self.entropy.read_range(0, 1024);
        let mut value = vec![0; length];
        self.values.fill_bytes(&mut value);
        value
    }

    fn interruption(
        &mut self,
        delay_map: Result<Vec<usize>, (usize, BufferStorage)>,
    ) -> StoreInterruption {
        if self.init.is_dirty() {
            // We only test that the store can power on without crashing. If it would get
            // interrupted then it's like powering up with a different initial state, which would be
            // tested with another fuzzing input.
            return StoreInterruption::none();
        }
        let delay_map = match delay_map {
            Ok(x) => x,
            Err((delay, storage)) => {
                print!("{}", storage);
                panic!("delay={}", delay);
            }
        };
        let delay = self.entropy.read_range(0, delay_map.len() - 1);
        let mut complete_bits = BitStack::default();
        for _ in 0..delay_map[delay] {
            complete_bits.push(self.entropy.read_bit());
        }
        if self.debug {
            if delay == delay_map.len() - 1 {
                assert!(complete_bits.is_empty());
                println!("Do not interrupt.");
            } else {
                println!(
                    "Interrupt after {} operations with complete mask {}.",
                    delay, complete_bits
                );
            }
        }
        if delay < delay_map.len() - 1 {
            self.increment(StatKey::InterruptionCount);
        }
        let corrupt = Box::new(move |old: &mut [u8], new: &[u8]| {
            for (old, new) in old.iter_mut().zip(new.iter()) {
                for bit in 0..8 {
                    let mask = 1 << bit;
                    if *old & mask == *new & mask {
                        continue;
                    }
                    if complete_bits.pop().unwrap() {
                        *old ^= mask;
                    }
                }
            }
        });
        StoreInterruption { delay, corrupt }
    }
}

enum Init {
    Clean,
    Dirty,
    Used { cycle: usize },
}

impl Init {
    fn is_dirty(&self) -> bool {
        match self {
            Init::Dirty => true,
            _ => false,
        }
    }

    fn used_cycles(&self) -> usize {
        match self {
            Init::Used { cycle } => *cycle,
            _ => 0,
        }
    }
}

#[derive(Default, Clone, Debug)]
struct BitStack {
    data: Vec<u8>,
    len: usize,
}

impl BitStack {
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn len(&self) -> usize {
        if self.len == 0 {
            8 * self.data.len()
        } else {
            8 * (self.data.len() - 1) + self.len
        }
    }

    fn push(&mut self, value: bool) {
        if self.len == 0 {
            self.data.push(0);
        }
        if value {
            *self.data.last_mut().unwrap() |= 1 << self.len;
        }
        self.len += 1;
        if self.len == 8 {
            self.len = 0;
        }
    }

    fn pop(&mut self) -> Option<bool> {
        if self.len == 0 {
            if self.data.len() == 0 {
                return None;
            }
            self.len = 8;
        }
        self.len -= 1;
        let result = self.data.last().unwrap() & 1 << self.len;
        if self.len == 0 {
            self.data.pop().unwrap();
        }
        Some(result != 0)
    }
}

impl std::fmt::Display for BitStack {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        let mut bits = self.clone();
        while let Some(bit) = bits.pop() {
            write!(f, "{}", bit as usize)?;
        }
        write!(f, " ({} bits)", self.len())?;
        Ok(())
    }
}

#[test]
fn bit_stack_ok() {
    let mut bits = BitStack::default();

    assert_eq!(bits.pop(), None);

    bits.push(true);
    assert_eq!(bits.pop(), Some(true));
    assert_eq!(bits.pop(), None);

    bits.push(false);
    assert_eq!(bits.pop(), Some(false));
    assert_eq!(bits.pop(), None);

    bits.push(true);
    bits.push(false);
    assert_eq!(bits.pop(), Some(false));
    assert_eq!(bits.pop(), Some(true));
    assert_eq!(bits.pop(), None);

    bits.push(false);
    bits.push(true);
    assert_eq!(bits.pop(), Some(true));
    assert_eq!(bits.pop(), Some(false));
    assert_eq!(bits.pop(), None);

    for i in 0..27 {
        assert_eq!(bits.len(), i);
        bits.push(true);
    }
}
