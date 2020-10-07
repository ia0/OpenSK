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

#![no_std]

extern crate alloc;
extern crate lang_items;

use alloc::vec;
use core::fmt::Write;
use ctap2::embedded_flash::SyscallStorage;
use libtock_drivers::console::Console;
use libtock_drivers::timer::{self, Duration, Timer, Timestamp};
use persistent_store::{Storage, Store};

const NUM_PAGES: usize = 3;
// This page size is for Nordic. It must be modified for other boards.
const PAGE_SIZE: usize = 0x1000;

fn timestamp(timer: &Timer) -> Timestamp<f64> {
    Timestamp::<f64>::from_clock_value(timer.get_current_clock().ok().unwrap())
}

fn measure<T>(timer: &Timer, operation: impl FnOnce() -> T) -> (T, Duration<f64>) {
    let before = timestamp(timer);
    let result = operation();
    let after = timestamp(timer);
    (result, after - before)
}

// Only use one store at a time.
unsafe fn boot_store(erase: bool) -> Store<SyscallStorage> {
    let mut storage = SyscallStorage::new(NUM_PAGES).unwrap();
    if erase {
        for page in 0..storage.num_pages() {
            storage.erase_page(page).unwrap();
        }
    }
    Store::new(storage).ok().unwrap()
}

#[derive(Copy, Clone, Debug)]
enum Type {
    Credential,
    Counter,
}

impl Type {
    fn expected_length(self) -> usize {
        match self {
            Type::Credential => 50,
            Type::Counter => 1,
        }
    }

    fn key_increment(self) -> usize {
        match self {
            Type::Credential => 1,
            Type::Counter => 0,
        }
    }
}

struct Stats {
    compaction: Duration<f64>,
    reboot: Duration<f64>,
}

fn measure_stats(timer: &Timer, typ_: Type) -> Stats {
    let mut store = unsafe { boot_store(true) };

    // Fill the store.
    store.insert(0, &[0; 4]).unwrap();
    let mut key = 1;
    let mut capacity = store.capacity().unwrap().remaining() - 2;
    while capacity > 1 {
        let length = core::cmp::min(capacity - 1, typ_.expected_length());
        store.insert(key, &vec![0; length * 4]).unwrap();
        key += typ_.key_increment();
        capacity -= 1 + length;
    }

    // Measure the reboot time.
    let (mut store, reboot) = measure(&timer, || unsafe { boot_store(false) });

    // Remove the first element. We will reinsert it to trigger exactly one compaction.
    store.remove(0).unwrap();

    // Measure the compaction time.
    let mut old_lifetime = store.lifetime().unwrap().used;
    let compaction = loop {
        let ((), time) = measure(timer, || store.insert(0, &[0; 4]).unwrap());
        let new_lifetime = store.lifetime().unwrap().used;
        if new_lifetime > old_lifetime + 2 {
            // We lost more lifetime than expected (there is at least the erase entry) which means a
            // compaction occurred.
            break time;
        }
        old_lifetime = new_lifetime;
    };

    Stats { compaction, reboot }
}

fn main() {
    let mut console = Console::new();
    let mut with_callback = timer::with_callback(|_, _| {});
    let timer = with_callback.init().ok().unwrap();

    write!(console, "Computing... ").unwrap();
    let counter = measure_stats(&timer, Type::Counter);
    let credential = measure_stats(&timer, Type::Credential);
    writeln!(console, "done.").unwrap();
    writeln!(
        console,
        "Compaction is about [{:.1} - {:.1}] ms.",
        counter.compaction.ms(),
        credential.compaction.ms()
    )
    .unwrap();
    writeln!(console, "Operations are about:").unwrap();
    // This is approximative but good enough.
    let m = unsafe { boot_store(false).max_value_length() } as f64 / PAGE_SIZE as f64;
    for n in 3..=20 {
        let ratio = (n as f64 - (1. + m)) / (2. - m);
        let min_reboot = credential.reboot.ms() * ratio;
        let max_reboot = counter.reboot.ms() * ratio;
        writeln!(
            console,
            "- [{:.1} - {:.1}] ms for {} pages.",
            min_reboot, max_reboot, n
        )
        .unwrap();
    }
    writeln!(
        console,
        "The best case is when the store has many credentials."
    )
    .unwrap();
    writeln!(
        console,
        "The worst case is when the store has few credentials."
    )
    .unwrap();
}
