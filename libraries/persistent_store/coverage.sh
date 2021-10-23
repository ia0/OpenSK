#!/bin/bash
#
# Usage: ./coverage.sh
set -e

readonly RUSTUP="$(rustup show home)"
readonly NIGHTLY="${RUSTUP}/toolchains/nightly-x86_64-unknown-linux-gnu"
readonly LLVM_COV="${NIGHTLY}/lib/rustlib/x86_64-unknown-linux-gnu/bin/llvm-cov"

if [[ ! -e "${NIGHTLY}" ]]; then
  rustup toolchain install nightly
fi

if [[ ! -e "${LLVM_COV}" ]]; then
  rustup component add --toolchain=nightly llvm-tools-preview
fi

if ! which cargo-fuzz >/dev/null 2>/dev/null; then
  cargo +nightly install cargo-fuzz
fi

PROFDATA="fuzz/coverage/store/coverage.profdata"

if [[ ! -e "${PROFDATA}" ]]; then
  cargo +nightly fuzz coverage store
fi

ARGS=(
  --ignore-filename-regex=cargo/registry
  --path-equivalence=src,fuzz/src
  --Xdemangler=rustfilt
  --instr-profile="${PROFDATA}"
  "fuzz/target/x86_64-unknown-linux-gnu/release/store"
)

"${LLVM_COV}" show --format=html "${ARGS[@]}" > coverage.html
"${LLVM_COV}" report "${ARGS[@]}"

echo
echo -e '\e[1;32mDone:\e[m coverage.html generated'
