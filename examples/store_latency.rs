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
unsafe fn boot_store(num_pages: usize, erase: bool) -> Store<SyscallStorage> {
    let mut storage = SyscallStorage::new(num_pages).unwrap();
    if erase {
        for page in 0..storage.num_pages() {
            storage.erase_page(page).unwrap();
        }
    }
    Store::new(storage).ok().unwrap()
}

fn compute_latency(timer: &Timer, num_pages: usize, key_increment: usize, word_length: usize) {
    let mut console = Console::new();
    writeln!(
        console,
        "\nLatency for num_pages={} key_increment={} word_length={}.",
        num_pages, key_increment, word_length
    )
    .unwrap();

    let mut store = unsafe { boot_store(num_pages, true) };
    let total_capacity = store.capacity().unwrap().total();

    // Burn N words to align the end of the user capacity with the virtual capacity.
    store.insert(0, &vec![0; 4 * (num_pages - 1)]).unwrap();
    store.remove(0).unwrap();

    // Insert entries until there is space for one more.
    let count = total_capacity / (1 + word_length) - 1;
    let ((), time) = measure(timer, || {
        for i in 0..count {
            let key = 1 + key_increment * i;
            // For some reason the kernel sometimes fails.
            while store.insert(key, &vec![0; 4 * word_length]).is_err() {
                // We never enter this loop in practice, but we still need it for the kernel.
                writeln!(console, "Retry insert.").unwrap();
            }
        }
    });
    writeln!(console, "Setup: {:.1}ms for {} entries.", time.ms(), count).unwrap();

    // Measure latency of insert.
    let key = 1 + key_increment * count;
    let ((), time) = measure(&timer, || {
        store.insert(key, &vec![0; 4 * word_length]).unwrap()
    });
    writeln!(console, "Insert: {:.1}ms.", time.ms()).unwrap();

    // Measure latency of boot.
    let (mut store, time) = measure(&timer, || unsafe { boot_store(num_pages, false) });
    writeln!(console, "Boot: {:.1}ms.", time.ms()).unwrap();

    // Measure latency of remove.
    let ((), time) = measure(&timer, || store.remove(key).unwrap());
    writeln!(console, "Remove: {:.1}ms.", time.ms()).unwrap();

    // Measure latency of compaction.
    let length = total_capacity + num_pages - store.lifetime().unwrap().used();
    if length > 0 {
        // Fill the store such that compaction is needed for one word.
        store.insert(0, &vec![0; 4 * (length - 1)]).unwrap();
        store.remove(0).unwrap();
    }
    let ((), time) = measure(timer, || store.prepare(1).unwrap());
    writeln!(console, "Compaction: {:.1}ms.", time.ms()).unwrap();
}

fn main() {
    let mut with_callback = timer::with_callback(|_, _| {});
    let timer = with_callback.init().ok().unwrap();

    writeln!(Console::new(), "\nRunning 4 tests...").unwrap();
    // Those non-overwritten 50 words entries simulate credentials.
    compute_latency(&timer, 3, 1, 50);
    compute_latency(&timer, 20, 1, 50);
    // Those overwritten 1 word entries simulate counters.
    compute_latency(&timer, 3, 0, 1);
    compute_latency(&timer, 20, 0, 1);
    writeln!(Console::new(), "\nDone.").unwrap();

    // Results on nrf52840dk:
    //
    // | Pages | Overwrite | Length    | Boot     | Compaction | Insert  | Remove |
    // | ----- | --------- | --------- | -------  | ---------- | ------  | ------ |
    // | 3     | no        | 50 words  | 2.0 ms   | 132.8 ms   | 4.3 ms  | 1.2 ms |
    // | 20    | no        | 50 words  | 7.8 ms   | 135.7 ms   | 9.9 ms  | 4.0 ms |
    // | 3     | yes       | 1 word    | 19.6 ms  | 90.8 ms    | 4.7 ms  | 2.3 ms |
    // | 20    | yes       | 1 word    | 183.3 ms | 90.9 ms    | 4.8 ms  | 2.3 ms |
}
