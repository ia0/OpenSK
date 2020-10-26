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

use crate::format::{Format, Position};
#[cfg(test)]
use crate::StoreUpdate;
use crate::{
    BufferCorruptFunction, BufferOptions, BufferStorage, Store, StoreError, StoreHandle,
    StoreModel, StoreOperation, StoreResult,
};

/// Tracks the store behavior against its model and its storage.
#[derive(Clone)]
pub enum StoreDriver {
    /// When the store is running.
    On(StoreDriverOn),

    /// When the store is off.
    Off(StoreDriverOff),
}

/// Keeps a store and its model in sync.
#[derive(Clone)]
pub struct StoreDriverOn {
    /// The store being tracked.
    store: Store<BufferStorage>,

    /// The model associated to the store.
    model: StoreModel,
}

#[derive(Clone)]
pub struct StoreDriverOff {
    storage: BufferStorage,
    model: StoreModel,
    /// Invariant if the interrupted operation would complete.
    complete: Option<Complete>,
}

#[derive(Clone)]
struct Complete {
    model: StoreModel,
    deleted: Vec<StoreHandle>,
}

pub struct StoreInterruption<'a> {
    pub delay: usize,
    pub corrupt: BufferCorruptFunction<'a>,
}

#[derive(Debug)]
pub enum StoreInvariant {
    NoLifetime,
    StoreError(StoreError),
    Interrupted {
        rollback: Box<StoreInvariant>,
        complete: Box<StoreInvariant>,
    },
    DifferentResult {
        store: StoreResult<()>,
        model: StoreResult<()>,
    },
    NotWiped {
        key: usize,
        value: Vec<u8>,
    },
    OnlyInStore {
        key: usize,
    },
    DifferentValue {
        key: usize,
        store: Box<[u8]>,
        model: Box<[u8]>,
    },
    OnlyInModel {
        key: usize,
    },
    DifferentCapacity {
        store: usize,
        model: usize,
    },
    DifferentErase {
        page: usize,
        store: usize,
        model: usize,
    },
    DifferentWrite {
        page: usize,
        word: usize,
        store: usize,
        model: usize,
    },
}

impl StoreDriver {
    pub fn storage(&self) -> &BufferStorage {
        match self {
            StoreDriver::On(x) => x.store().storage(),
            StoreDriver::Off(x) => x.storage(),
        }
    }

    pub fn on(self) -> Option<StoreDriverOn> {
        match self {
            StoreDriver::On(x) => Some(x),
            StoreDriver::Off(_) => None,
        }
    }

    pub fn power_on(self) -> Result<StoreDriverOn, StoreInvariant> {
        match self {
            StoreDriver::On(x) => Ok(x),
            StoreDriver::Off(x) => x.power_on(),
        }
    }

    pub fn off(self) -> Option<StoreDriverOff> {
        match self {
            StoreDriver::On(_) => None,
            StoreDriver::Off(x) => Some(x),
        }
    }
}

impl StoreDriverOff {
    pub fn new(options: BufferOptions, num_pages: usize) -> StoreDriverOff {
        let storage = vec![0xff; num_pages * options.page_size].into_boxed_slice();
        let storage = BufferStorage::new(storage, options);
        StoreDriverOff::new_dirty(storage)
    }

    pub fn new_dirty(storage: BufferStorage) -> StoreDriverOff {
        let format = Format::new(&storage).unwrap();
        StoreDriverOff {
            storage,
            model: StoreModel::new(format),
            complete: None,
        }
    }

    pub fn storage(&self) -> &BufferStorage {
        &self.storage
    }

    pub fn storage_mut(&mut self) -> &mut BufferStorage {
        &mut self.storage
    }

    pub fn model(&self) -> &StoreModel {
        &self.model
    }

    pub fn power_on(self) -> Result<StoreDriverOn, StoreInvariant> {
        Ok(self
            .partial_power_on(StoreInterruption::none())
            .map_err(|x| x.1)?
            .on()
            .unwrap())
    }

    pub fn partial_power_on(
        mut self,
        interruption: StoreInterruption,
    ) -> Result<StoreDriver, (BufferStorage, StoreInvariant)> {
        self.storage.arm_interruption(interruption.delay);
        Ok(match Store::new(self.storage) {
            Ok(mut store) => {
                store.storage_mut().disarm_interruption();
                let mut error = None;
                if let Some(complete) = self.complete {
                    match StoreDriverOn::new(store, complete.model, &complete.deleted) {
                        Ok(driver) => return Ok(StoreDriver::On(driver)),
                        Err((e, x)) => {
                            error = Some(e);
                            store = x;
                        }
                    }
                };
                StoreDriver::On(StoreDriverOn::new(store, self.model, &[]).map_err(
                    |(rollback, store)| {
                        let storage = store.into_storage();
                        match error {
                            None => (storage, rollback),
                            Some(complete) => {
                                let rollback = Box::new(rollback);
                                let complete = Box::new(complete);
                                (storage, StoreInvariant::Interrupted { rollback, complete })
                            }
                        }
                    },
                )?)
            }
            Err((StoreError::StorageError, mut storage)) => {
                storage.corrupt_operation(interruption.corrupt);
                StoreDriver::Off(StoreDriverOff { storage, ..self })
            }
            Err((error, mut storage)) => {
                storage.reset_interruption();
                return Err((storage, StoreInvariant::StoreError(error)));
            }
        })
    }

    /// Returns a mapping from delay time to number of modified bits.
    pub fn delay_map(&self) -> Result<Vec<usize>, (usize, BufferStorage)> {
        let mut result = Vec::new();
        loop {
            let delay = result.len();
            let mut storage = self.storage.clone();
            storage.arm_interruption(delay);
            match Store::new(storage) {
                Err((StoreError::StorageError, x)) => storage = x,
                Err((StoreError::InvalidStorage, mut storage)) => {
                    storage.reset_interruption();
                    return Err((delay, storage));
                }
                Ok(_) | Err(_) => break,
            }
            result.push(count_modified_bits(&mut storage));
        }
        result.push(0);
        Ok(result)
    }
}

impl StoreDriverOn {
    pub fn store(&self) -> &Store<BufferStorage> {
        &self.store
    }

    pub fn into_store(self) -> Store<BufferStorage> {
        self.store
    }

    pub fn store_mut(&mut self) -> &mut Store<BufferStorage> {
        &mut self.store
    }

    pub fn model(&self) -> &StoreModel {
        &self.model
    }

    pub fn apply(&mut self, operation: StoreOperation) -> Result<(), StoreInvariant> {
        let (deleted, store_result) = self.store.apply(&operation);
        let model_result = self.model.apply(operation);
        if store_result != model_result {
            return Err(StoreInvariant::DifferentResult {
                store: store_result,
                model: model_result,
            });
        }
        self.check_deleted(&deleted)?;
        Ok(())
    }

    pub fn partial_apply(
        mut self,
        operation: StoreOperation,
        interruption: StoreInterruption,
    ) -> Result<(Option<StoreError>, StoreDriver), (Store<BufferStorage>, StoreInvariant)> {
        self.store
            .storage_mut()
            .arm_interruption(interruption.delay);
        let (deleted, store_result) = self.store.apply(&operation);
        Ok(match store_result {
            Err(StoreError::NoLifetime) => return Err((self.store, StoreInvariant::NoLifetime)),
            Ok(()) | Err(StoreError::NoCapacity) | Err(StoreError::InvalidArgument) => {
                self.store.storage_mut().disarm_interruption();
                let model_result = self.model.apply(operation);
                if store_result != model_result {
                    return Err((
                        self.store,
                        StoreInvariant::DifferentResult {
                            store: store_result,
                            model: model_result,
                        },
                    ));
                }
                if store_result.is_ok() {
                    if let Err(invariant) = self.check_deleted(&deleted) {
                        return Err((self.store, invariant));
                    }
                }
                (store_result.err(), StoreDriver::On(self))
            }
            Err(StoreError::StorageError) => {
                let mut driver = StoreDriverOff {
                    storage: self.store.into_storage(),
                    model: self.model,
                    complete: None,
                };
                driver.storage.corrupt_operation(interruption.corrupt);
                let mut model = driver.model.clone();
                if model.apply(operation).is_ok() {
                    driver.complete = Some(Complete { model, deleted });
                }
                (None, StoreDriver::Off(driver))
            }
            Err(error) => return Err((self.store, StoreInvariant::StoreError(error))),
        })
    }

    pub fn delay_map(
        &self,
        operation: &StoreOperation,
    ) -> Result<Vec<usize>, (usize, BufferStorage)> {
        let mut result = Vec::new();
        loop {
            let delay = result.len();
            let mut store = self.store.clone();
            store.storage_mut().arm_interruption(delay);
            match store.apply(operation).1 {
                Err(StoreError::StorageError) => (),
                Err(StoreError::InvalidStorage) => return Err((delay, store.into_storage())),
                Ok(()) | Err(_) => break,
            }
            result.push(count_modified_bits(store.storage_mut()));
        }
        result.push(0);
        Ok(result)
    }

    pub fn power_off(self) -> StoreDriverOff {
        StoreDriverOff {
            storage: self.store.into_storage(),
            model: self.model,
            complete: None,
        }
    }

    #[cfg(test)]
    pub fn insert(&mut self, key: usize, value: &[u8]) -> Result<(), StoreInvariant> {
        let value = value.to_vec();
        let updates = vec![StoreUpdate::Insert { key, value }];
        self.apply(StoreOperation::Transaction { updates })
    }

    #[cfg(test)]
    pub fn remove(&mut self, key: usize) -> Result<(), StoreInvariant> {
        let updates = vec![StoreUpdate::Remove { key }];
        self.apply(StoreOperation::Transaction { updates })
    }

    pub fn check(&self) -> Result<(), StoreInvariant> {
        self.recover_check(&[])
    }

    fn new(
        store: Store<BufferStorage>,
        model: StoreModel,
        deleted: &[StoreHandle],
    ) -> Result<StoreDriverOn, (StoreInvariant, Store<BufferStorage>)> {
        let driver = StoreDriverOn { store, model };
        match driver.recover_check(deleted) {
            Ok(()) => Ok(driver),
            Err(error) => Err((error, driver.store)),
        }
    }

    fn recover_check(&self, deleted: &[StoreHandle]) -> Result<(), StoreInvariant> {
        self.check_deleted(deleted)?;
        self.check_model()?;
        self.check_storage()?;
        Ok(())
    }

    fn check_deleted(&self, deleted: &[StoreHandle]) -> Result<(), StoreInvariant> {
        for handle in deleted {
            let value = self.store.inspect_value(&handle);
            if !value.iter().all(|&x| x == 0x00) {
                return Err(StoreInvariant::NotWiped {
                    key: handle.get_key(),
                    value,
                });
            }
        }
        Ok(())
    }

    fn check_model(&self) -> Result<(), StoreInvariant> {
        let mut model_map = self.model.map().clone();
        for handle in self.store.iter().unwrap() {
            let handle = handle.unwrap();
            let model_value = match model_map.remove(&handle.get_key()) {
                None => {
                    return Err(StoreInvariant::OnlyInStore {
                        key: handle.get_key(),
                    })
                }
                Some(x) => x,
            };
            let store_value = handle.get_value(&self.store).unwrap().into_boxed_slice();
            if store_value != model_value {
                return Err(StoreInvariant::DifferentValue {
                    key: handle.get_key(),
                    store: store_value,
                    model: model_value,
                });
            }
        }
        if let Some(&key) = model_map.keys().next() {
            return Err(StoreInvariant::OnlyInModel { key });
        }
        let store_capacity = self.store.capacity().unwrap().remaining();
        let model_capacity = self.model.capacity().remaining();
        if store_capacity != model_capacity {
            return Err(StoreInvariant::DifferentCapacity {
                store: store_capacity,
                model: model_capacity,
            });
        }
        Ok(())
    }

    fn check_storage(&self) -> Result<(), StoreInvariant> {
        let format = self.model.format();
        let storage = self.store.storage();
        let num_words = format.page_size() / format.word_size();
        let head = self.store.head().unwrap();
        let tail = self.store.tail().unwrap();
        for page in 0..format.num_pages() {
            // Check the erase cycle of the page.
            let store_erase = head.cycle(format) + (page < head.page(format)) as usize;
            let model_erase = storage.get_page_erases(page);
            if store_erase != model_erase {
                return Err(StoreInvariant::DifferentErase {
                    page,
                    store: store_erase,
                    model: model_erase,
                });
            }
            let page_pos = Position::new(format, model_erase, page, 0);

            // Check the init word of the page.
            let mut store_write = (page_pos < tail) as usize;
            if page == 0 && tail == Position::new(format, 0, 0, 0) {
                // When the store is initialized and nothing written yet, the first page is still
                // initialized.
                store_write = 1;
            }
            let model_write = storage.get_word_writes(page * num_words);
            if store_write != model_write {
                return Err(StoreInvariant::DifferentWrite {
                    page,
                    word: 0,
                    store: store_write,
                    model: model_write,
                });
            }

            // Check the compact info of the page.
            let model_write = storage.get_word_writes(page * num_words + 1);
            let store_write = 0;
            if store_write != model_write {
                return Err(StoreInvariant::DifferentWrite {
                    page,
                    word: 1,
                    store: store_write,
                    model: model_write,
                });
            }

            // Check the content of the page. We only check cases where the model says a word was
            // written while the store doesn't think it should be the case. This is because the
            // model doesn't count writes to the same value. Also we only check whether a word is
            // written and not how many times. This is because this is hard to rebuild in the store.
            for word in 2..num_words {
                let store_write = (page_pos + (word - 2) < tail) as usize;
                let model_write = (storage.get_word_writes(page * num_words + word) > 0) as usize;
                if store_write < model_write {
                    return Err(StoreInvariant::DifferentWrite {
                        page,
                        word,
                        store: store_write,
                        model: model_write,
                    });
                }
            }
        }
        Ok(())
    }
}

impl<'a> StoreInterruption<'a> {
    pub fn none() -> StoreInterruption<'a> {
        StoreInterruption {
            delay: usize::max_value(),
            corrupt: Box::new(|_, _| {}),
        }
    }
}

fn count_modified_bits(storage: &mut BufferStorage) -> usize {
    let mut modified_bits = 0;
    storage.corrupt_operation(Box::new(|before, after| {
        modified_bits = before
            .iter()
            .zip(after.iter())
            .map(|(x, y)| (x ^ y).count_ones() as usize)
            .sum();
    }));
    // We should never write the same slice or erase an erased page.
    assert!(modified_bits > 0);
    modified_bits
}
