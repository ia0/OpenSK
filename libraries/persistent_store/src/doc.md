# Specification

The store provides a partial function from keys to values on top of a storage
interface. The store total capacity depends on the size of the storage. Store
updates may be bundled in transactions. Mutable operations are atomic, including
when interrupted.

The store is flash-efficient in the sense that it uses the storage lifetime
efficiently. For each page, all words are written at least once between erase
cycles and all erase cycles are used. However, not all written words are user
content: lifetime is also consumed with metadata and compaction.

The store is extendable with other entries than key-values. It is essentially a
framework providing access to the storage lifetime. The partial function is
simply the most common usage and can be used to encode other usages.

## Definitions

An _entry_ is a pair of a key and a value. A _key_ is a number between 0
and 4095. A _value_ is a byte slice with a length between 0 and 1023 bytes (for
large enough pages).

The store provides the following _updates_:
-   Given a key and a value, `Insert` updates the store such that the value is
    associated with the key. The value for other keys are left unchanged.
-   Given a key, `Remove` updates the store such that no value is associated for
    the key. The value for other keys are left unchanged. Additionally, if there
    was a value associated with the key, the value is wiped from the storage
    (all its bits are set to 0).

The store provides the following _read-only operations_:
-   `Iter` iterates through the store returning all entries exactly once. The
    iteration order is not specified but stable between mutable operations.
-   `Capacity` returns how many words can be stored before the store is full.
-   `Lifetime` returns how many words can be written before the storage lifetime
    is consumed.

The store provides the following _mutable operations_:
-   Given a set of independent updates, `Transaction` applies the sequence of
    updates.
-   Given a threshold, `Clear` removes all entries with a key greater or equal
    to the threshold.
-   Given a length in words, `Prepare` makes one step of compaction unless that
    many words can be written without compaction. This operation has no effect
    on the store but may still mutate its storage. In particular, the store has
    the same capacity but a possibly reduced lifetime.

A mutable operation is _atomic_ if, when power is lost during the operation, the
store is either updated (as if the operation succeeded) or left unchanged (as if
the operation did not occur). If the store is left unchanged, lifetime may still
be consumed.

The store relies on the following _storage interface_:
-   It is possible to read a byte slice. The slice won't span multiple pages.
-   It is possible to write a word slice. The slice won't span multiple pages.
-   It is possible to erase a page.
-   The pages are sequentially indexed from 0. If the actual underlying storage
    is segmented, then the storage layer should translate those indices to
    actual page addresses.

The store has a _total capacity_ of `C = (N - 1) * (P - 4) - M - 1` words, where
`P` is the number of words per page, `N` is the number of pages, and `M` is the
maximum length in words of a value (256 for large enough pages). The capacity
used by each mutable operation is given below (a transient word only uses
capacity during the operation):
-   `Insert` uses `1 + ceil(len / 4)` words where `len` is the length of the
    value in bytes. If an entry was replaced, the words used by its insertion
    are freed.
-   `Remove` doesn't use capacity if alone in the transaction and 1 transient
    word otherwise. If an entry was deleted, the words used by its insertion are
    freed.
-   `Transaction` uses 1 transient word. In addition, the updates of the
    transaction use and free words as described above.
-   `Clear` doesn't use capacity and frees the words used by the insertion of
    the deleted entries.
-   `Prepare` doesn't use capacity.

The _total lifetime_ of the store is below `L = ((E + 1) * N - 1) * (P - 2)` and
above `L - M` words, where `E` is the maximum number of erase cycles. The
lifetime is used when capacity is used, including transiently, as well as when
compaction occurs. The more the store is loaded (few remaining words of
capacity), the more compactions are frequent, and the more lifetime is used.

It is possible to approximate the cost of transient words in terms of capacity:
`L` transient words are equivalent to `C - x` words of capacity where `x` is the
average capacity (including transient) of operations.

## Preconditions

The store may behave in unexpected ways if the following assumptions don't hold:
-   A word can be written twice between erase cycles.
-   A page can be erased `E` times after the first boot of the store.
-   When power is lost while writing a slice or erasing a page, the next read
    returns a slice where a subset (possibly none or all) of the bits that
    should have been modified have been modified.
-   Reading a slice is deterministic. When power is lost while writing a slice
    or erasing a slice (erasing a page containing that slice), reading that
    slice repeatedly returns the same result (until it is overwritten or its
    page is erased).
-   To decide whether a page has been erased, it is enough to test if all its
    bits are equal to 1.
-   When power is lost while writing a slice or erasing a page, that operation
    does not count towards the limits. However, completing that write or erase
    operation would count towards the limits, as if the number of writes per
    word and number of erase cycles could be fractional.
-   The storage is only modified by the store. Note that completely erasing the
    storage is supported, essentially losing all content and lifetime tracking.
    It is preferred to use `Clear` with a threshold of 0 to keep the lifetime
    tracking.

The store properties may still hold outside some of those assumptions but with
weaker probabilities as the usage diverges from them.

According to [Understanding the Impact of Power Loss on Flash Memory][PowerCut],
we estimate those assumptions hold 99.9999% (about 20 bits) of the time when SLC
cells are used:
-   Out of the 6 different SLC chips tested, only one could turn a bit from 0 to
    1 during write (but the paper doesn't say if it's a bit that used to be
    equal to 1 before the write).
-   Reading is deterministic for the SLC chip tested and stable with aging (up
    to 10 years). The bit error rate is 0 for almost all cases. If the power
    loss occurs very shortly after the write starts, the bit error rate is
    0.00001% (about 23 bits).
-   Out of the 6 SLC chips tested, all behave correctly for partial erase
    operations: bits are only switched from 0 to 1 and writing over the partial
    erase doesn't show errors.

[PowerCut]: https://cseweb.ucsd.edu/~swanson/papers/DAC2011PowerCut.pdf

## Properties

### Memory

The store uses 3 words in addition to the size of the storage object. Read-only
operations don't allocate. Mutable operations allocate the written capacity. For
transaction, this is done sequentially, so the previous allocation is freed
before the next allocation. Compaction allocates the capacity of each entry
being copied sequentially. Transaction also allocates the number of keys in the
transaction which is by default at most 31.

So all operations allocate at most 257 words at a time for large enough pages.

### Latency

All operations are at most linear in the storage. When compaction occurs, the
latency is still linear but with probably a larger constant due to writes being
probably slower than reads. It is also possible that the storage is almost
compacted and requires multiple steps of compaction (at most `N - 1`) until the
remaining capacity is available.

Note that it may be important to regularly call `Prepare` when the device is
idle to improve the latency of the next mutable operation, since it would reduce
or eliminate (if enough capacity was prepared) the need for compaction. This is
important because for some use-cases (when transient words are clustered)
multiple compaction may be needed before a mutable operation can proceed.

#### Measurements on nRF52840

nRF52840 operations:
-   Erasing a page takes 85ms.
-   Writing a word takes 41us.
-   Reading a word takes about 11us (not specified).

Tock operations:
-   A syscall takes about 85us.

Store operations:
-   Booting the store takes between 20ms (when the store is filled with entries
    of 50 words) and 200ms (when the store is filled with entries of 2 words)
    for 20 pages.
-   Compacting a page takes between 100ms (when the page is filled with deleted
    entries of 2 words) and 150ms (when the page is filled with entries of 50
    words that need to be copied).
-   Inserting and removing entries is similar to booting the store.

## Examples

### Migration

The content of the store may be migrated if the application only uses keys 1
to 2047. Key 0 stores a version number. Keys 2048 to 4095 are reserved for
migration purposes.

The state of a migration is defined by the keys 0 and 2048:
-   If only key 0 is present, there is no ongoing migration. The application may
    use the store.
-   If both key 0 and 2048 are present, some entries are being migrated from key
    `old_key` to `2048 + new_key` and from the version in key 0 to the version
    in key 2048.
-   If only key 2048 is present, some entries are being copied from key `2048 +
    key` to `key`.

The migration steps are:
-   Insert key 2048 with the new version.
-   Migrate the entries for which the key or the value would differ (in store
    order to minimize compaction).
-   Remove key 0.
-   Copy the entries back to their original key (in store order too).
-   Move key 2048 to key 0.

Applications should check that key 0 is present with a supported version and key
2048 is absent.

Migration applications should check that either:
-   Key 0 is present with a supported version and key 2048 is absent.
-   Key 0 is present with a supported source version and key 2048 is present
    with a supported target version.
-   Key 0 is absent and key 2048 is present (no matter the version).

It should actually be possible to migrate the entry format (maybe even the page
format) by following these steps:
-   Fully compact the store before the migration to avoid need of compaction
    during the migration.
-   Write a migration separator `0x83ffffff` (i.e. the longest prefix with no
    information and a checksum). Entries and pages before and including this
    position are in the old format. Entries and pages after this position are in
    the new format.
-   Migrate the content before to after the migration separator. If the page
    format changes, the rest of the page after the migration separator should be
    skipped. There should be a way to differentiate the old from the new page
    format.
-   If the page format has changed, erase the pages before and including the
    separator. Otherwise, pad the separator indicating the end of the migration.

### Fragmentation

Support to write values longer than 1023 bytes could be added on top of the
store by fragmenting long entries. All fragments are updated as part of the same
transaction.

## Extensions

Extensions can be added provided compaction continue to preserve the semantics
of the store. In particular, compaction should preserve the capacity and it
should not use transient capacity without adjusting the virtual storage size
accordingly.

### Counters

We can add 256 counters in addition to the 4096 keys. The counters are 32-bits
and can be configured to have wrapping or saturating semantics. They are
identified with a counter id between 0 and 255. This extension doesn't change
the cost of a counter, it is still 2 words, but it reduces the cost of
increments by 4 times at best and 2 times at worst.

They provide the following read-only operations:
-   Given a counter id, `Read` returns the value of the counter. If the counter
    does not exist, an error is returned.

They provide the following mutable operations:
-   Given a counter id, `Increment` increments the value of the counter
    according to its wrapping or saturating semantics. If the counter does not
    exist, lifetime is consumed but no error is returned.
-   Given a counter id, a value, and an wrapping or saturating semantics,
    `Create` sets the counter to the value with the semantics. If the counter
    was already present, it is replaced.

The capacity used by each mutable operation is given below:
-   Every 2 `Increment` (starting with the first) uses 1 transient word.
-   `Create` uses 2 words. If the counter was replaced, those words are
    transient.

The implementation uses a half-word for increments with 4 bits checksum. When a
counter is compacted, it's value is updated according to the increments between
its old and new position.

### Isolation

It is possible to provide user isolation with the following setup:
-   Each user is uniquely associated with a number between 0 and `MAX_USER`.
    This number should be stable through the lifetime of the store.
-   `capacity(user)` is a function mapping `user` to the capacity it may use.
    The cumulative capacity for all users should not exceed the store capacity.
-   `lifetime(user)` is a function mapping `user` to the lifetime it may use.
    The cumulative lifetime for all users should not exceed the store lifetime.
-   `access(user, permission, key)` is a relation defining whether `user` has
    `permission` on `key`. A permission is either `ReadOnly` or `ReadWrite`.

It is easy to check access without store support and to some extent compute user
capacity. However, the `Lifetime` operation needs store support. For each user,
there is an entry tracking its lifetime usage. Before each operation writing to
the virtual storage, a marker is written with the user responsible for the
operation. When a tracker is compacted, its value is updated according to the
markers between its old and new position. The lifetime added by a marker is the
number of words up to the next marker. A marker is written before compaction so
that the compaction is accounted to the user triggering it.

The new entries are:
-   Tracker: 2 words containing the following fields:
    -   The user number.
    -   The lifetime used by this user.
    -   A bit indicating whether this tracker also counts as a marker. This is
        set when the tracker is written during a compaction triggered by the
        same user.
    -   A checksum.
-   Marker: A word containing the user number and a checksum.

## Related work

### OpenSK store

The main differences with the current OpenSK store are:
-   The possibility to have transactions.
-   Interface is a map instead of a multi-set.
-   Words are not assumed to be written or erased atomically and sequentially.
-   Overhead is more compact when an entry is replaced: at least 5 bits less
    which permits to gain 1 word due to alignment restrictions when the value is
    word-aligned which is the case for counters.
-   The linear virtual storage permits to more easily compute properties due to
    lack of fragmentation.
-   Values may not be stored contiguously when spanning 2 pages. Reading a value
    doesn't return a slice but a vector.
-   Adding new kinds of keys doesn't change the format and thus doesn't
    invalidate the store.

### CR50 store

The main differences with the [CR50 store] are:
-   All interrupted operations are correctly handled. Since CR50 uses a prefix
    of SHA1 for the checksum, the probability of detection is only 99.98% (12
    bits). Note however that if interrupted operations may modify bits that
    should not have been modified, the CR50 probability stays the same, while
    the store would drop from 100% to 97% (5 bits) for those cases. The overall
    probability would be 99.98% if those cases frequency is 1% (7 bits).
-   Transactions with a single update don't need a delimiter.
-   Keys are numbers instead of byte slices and they are part of the 1 word
    overhead.
-   Values can be 1023 bytes long instead of 255 and can be empty. In CR50,
    inserting an entry with an empty value deletes the previous entry.
-   There is no TPM-specific support.
-   Keys and values are not encrypted. Values can be encrypted by the user
    before being stored.
-   Compaction is incremental: pages are compacted one at a time until enough
    capacity is made available. CR50 compacts all written pages at once.
-   Compaction doesn't shuffle pages. Pages are always in storage cyclic order.
-   Only 1.5 pages are reserved for compaction instead of 2.9.

[CR50 store]: https://chromium.googlesource.com/chromiumos/platform/ec/+/refs/heads/cr50_stab/common/new_nvmem.c

### True2F counters

The [True2F counters] are a very specific usage, but we can compare with a
similar store. We assume 50000 erase cycles for 3 pages of 512 words. We
configure a maximum value length of 1 word. Counters are simply entries whose
value is the value of the counter and key is a number. The main differences are:
-   Counters are indexed by a key instead of a hash. As such, they use 2 words
    instead of 5.
-   The full lifetime of the 3 pages is used. The True2F counters only use half
    of the lifetime of the 2 data pages. The log page is still used fully. So
    True2F counters use the lifetime of 2 pages.
-   There is no overflow counter and all counters have a precise value. True2F
    counters are an over-approximation of the number of increments.
-   The number of counters is linked to the number of increments. For True2F,
    the number of counters is independent of the number of increments (less
    counters doesn't increase the number of increments and reciprocally). There
    can be up to 100 True2F counters and from 6M to 50M increments. For the
    store, with `x <= 500` counters, the number of increments is `(1000 - 2*x) *
    75000`. So for 100 counters it would be 60M increments. Using the counter
    extension would improve the number of increments by 2 to 4 times but may
    also reduce the maximum number of counters to 256.

Note that the True2F counters can be improved to reach similar performance:
-   The number of increments can be improved by a factor of 1.5 times by using
    the lifetime of 3 pages instead of 2. This can be achieved by considering
    the 3 pages as a cyclic resource. The pages are always in this order: log
    page, data page, blank page. At each compaction, the current blank page is
    written as the new data page, then the current log page and current data
    page are erased and become the new blank page and new log page respectively.
-   The number of counters can be improved by a factor of 2.5 times by using
    keys instead of hashes. This is only possible if a mapping from credentials
    to keys is available, for example by storing the counter key in the
    credential itself. This would also improve or eliminate the worst case
    scenario for the number of increments depending on the size of the keys.

[True2F counters]: https://arxiv.org/abs/1810.04660

# Implementation

We define the following constants:
-   `E < 65536` the number of times a page can be erased.
-   `3 <= N < 64` the number of pages in the storage.
-   `8 <= P <= 1024` the number of words in a page.
-   `Q = P - 2` the number of words in a virtual page.
-   `K = 4096` the maximum number of keys.
-   `M = min(Q - 1, 256)` the maximum length in words of a value.
-   `V = (N - 1) * (Q - 1) - M` the virtual capacity.
-   `C = V - N` the user capacity.

We build a virtual storage from the physical storage using the first 2 words of
each page:
-   The first word contains the number of times the page has been erased.
-   The second word contains the starting word to which this page is being moved
    during compaction.

The virtual storage has a length of `(E + 1) * N * Q` words and represents the
lifetime of the store. (We reserve the last `Q + M` words to support adding
emergency lifetime.) This virtual storage has a linear address space.

We define a set of overlapping windows of `N * Q` words at each `Q`-aligned
boundary. We call `i` the window spanning from `i * Q` to `(i + N) * Q`. Only
those windows actually exist in the underlying storage. We use compaction to
shift the current window from `i` to `i + 1`, preserving the content of the
store.

For a given state of the virtual storage, we define `h_i` as the position of the
first entry of the window `i`. We call it the head of the window `i`. Because
entries are at most `M + 1` words, they can overlap on the next page only by `M`
words. So we have `i * Q <= h_i <= i * Q + M` . Since there are no entries
before the first page, we have `h_0 = 0`.

We define `t_i` as one past the last entry of the window `i`. If there are no
entries in that window, we have `t_i = h_i`. We call `t_i` the tail of the
window `i`. We define the compaction invariant as `t_i - h_i <= V`.

We define `|x|` as the capacity used before position `x`. We have `|x| <= x`. We
define the capacity invariant as `|t_i| - |h_i| <= C`.

Using this virtual storage, entries are appended to the tail as long as there is
both virtual capacity to preserve the compaction invariant and capacity to
preserve the capacity invariant. When virtual capacity runs out, the first page
of the window is compacted and the window is shifted.

Entries are identified by a prefix of bits. The prefix has to contain at least
one bit set to zero to differentiate from the tail. Entries can be one of:
-   Padding: A word whose first bit is set to zero. The rest is arbitrary. This
    entry is used to mark words partially written after an interrupted operation
    as padding such that they are ignored by future operations.
-   Header: A word whose second bit is set to zero. It contains the following fields:
    -   A bit indicating whether the entry is deleted.
    -   A bit indicating whether the value is word-aligned and has all bits set
        to 1 in its last word. The last word of an entry is used to detect that
        an entry has been fully written. As such it must contain at least one
        bit equal to zero.
    -   The key of the entry.
    -   The length in bytes of the value. The value follows the header. The
        entry is word-aligned if the value is not.
    -   The checksum of the first and last word of the entry.
-   Erase: A word used during compaction. It contains the page to be erased and
    a checksum.
-   Clear: A word used during the `Clear` operation. It contains the threshold
    and a checksum.
-   Marker: A word used during the `Transaction` operation. It contains the
    number of updates following the marker and a checksum.
-   Remove: A word used during the `Transaction` operation. It contains the key
    of the entry to be removed and a checksum.

Checksums are the number of bits equal to 0.

# Proofs

## Compaction

It should always be possible to fully compact the store, after what the
remaining capacity should be available in the current window (restoring the
compaction invariant). We consider all notations on the virtual storage after
the full compaction. We will use the `|x|` notation although we update the state
of the virtual storage. This is fine because compaction doesn't change the
status of an existing word.

We want to show that the next `N - 1` compactions won't move the tail past the
last page of their window, with `I` the initial window:

```
forall 1 <= i <= N - 1, t_{I + i} <= (I + i + N - 1) * Q
```

We assume `i` between `1` and `N - 1`.

One step of compaction advances the tail by how many words were used in the
first page of the window with the last entry possibly overlapping on the next
page.

```
forall j, t_{j + 1} = t_j + |h_{j + 1}| - |h_j| + 1
```

By induction, we have:

```
t_{I + i} <= t_I + |h_{I + i}| - |h_I| + i
```

We have the following properties:

```
t_I <= h_I + V
|h_{I + i}| - |h_I| <= h_{I + i} - h_I
h_{I + i} <= (I + i) * Q + M
```

Replacing into our previous equality, we can conclude:

```
t_{I + i}  = t_I + |h_{I + i}| - |h_I| + i
          <= h_I + V + (I + i) * Q + M - h_I + i
           = (N - 1) * (Q - 1) - M + (I + i) * Q + M + i
           = (N - 1) * (Q - 1) + (I + i) * Q + i
           = (I + i + N - 1) * Q + i - (N - 1)
          <= (I + i + N - 1) * Q
```

We also want to show that after `N - 1` compactions, the remaining capacity is
available without compaction.

```
V - (t_{I + N - 1} - h_{I + N - 1}) >=    // The available words in the window.
  C - (|t_{I + N - 1}| - |h_{I + N - 1}|) // The remaining capacity.
  + 1                                     // Reserved for Clear.
```

We can replace the definition of `C` and simplify:

```
V - (t_{I + N - 1} - h_{I + N - 1}) >= V - N - (|t_{I + N - 1}| - |h_{I + N - 1}|) + 1
iff t_{I + N - 1} - h_{I + N - 1} <= |t_{I + N - 1}| - |h_{I + N - 1}| + N - 1
```

We have the following properties:

```
t_{I + N - 1} = t_I + |h_{I + N - 1}| - |h_I| + N - 1
|t_{I + N - 1}| - |h_{I + N - 1}| = |t_I| - |h_I| // Compaction preserves capacity.
|h_{I + N - 1}| - |t_I| <= h_{I + N - 1} - t_I
```

From which we conclude:

```
t_{I + N - 1} - h_{I + N - 1} <= |t_{I + N - 1}| - |h_{I + N - 1}| + N - 1
iff t_I + |h_{I + N - 1}| - |h_I| + N - 1 - h_{I + N - 1} <= |t_I| - |h_I| + N - 1
iff t_I + |h_{I + N - 1}| - h_{I + N - 1} <= |t_I|
iff |h_{I + N - 1}| - |t_I| <= h_{I + N - 1} - t_I
```


## Checksum

The main property we want is that all partially written/erased words are either
the initial word, the final word, or invalid.

We say that a bit sequence `TARGET` is reachable from a bit sequence `SOURCE` if
both have the same length and `SOURCE & TARGET == TARGET` where `&` is the
bitwise AND operation on bit sequences of that length. In other words, when
`SOURCE` has a bit equal to 0 then `TARGET` also has that bit equal to 0.

The only written entries start with `101` or `110` and are written from an
erased word. Marking an entry as padding or deleted is a single bit operation,
so the property trivially holds. For those cases, the proof relies on the fact
that there is exactly one bit equal to 0 in the 3 first bits. Either the 3 first
bits are still `111` in which case we expect the remaining bits to bit equal
to 1. Otherwise we can use the checksum of the given type of entry because those
2 types of entries are not reachable from each other.

To show that valid entries of a given type are not reachable from each other, we
show 3 lemmas:

1.  A bit sequence is not reachable from another if its number of bits equal to
    0 is smaller.

2.  A bit sequence is not reachable from another if they have the same number of
    bits equals to 0 and are different.

3.  A bit sequence is not reachable from another if it is bigger when they are
    interpreted as numbers in binary representation.

From those lemmas we consider the 2 cases. If both entries have the same number
of bits equal to 0, they are either equal or not reachable from each other
because of the second lemma. If they don't have the same number of bits equal to
0, then the one with less bits equal to 0 is not reachable from the other
because of the first lemma and the one with more bits equal to 0 is not
reachable from the other because of the third lemma and the definition of the
checksum.

# Fuzzing

For any sequence of operations and interruptions starting from an erased
storage, the store is checked against its model and some internal invariant at
each step.

For any sequence of operations and interruptions starting from an arbitrary
storage, the store is checked not to crash.
