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

use fuzz_store::{fuzz, Stats};
use std::io::Write;
use std::io::{stdout, Read};
use std::path::Path;

fn usage(program: &str) {
    println!(
        r#"Usage: {} {{ [<artifact_file>] | <corpus_directory> }}

If <artifact_file> is not provided, it is read from standard input."#,
        program
    );
}

fn debug(data: &[u8]) {
    println!("{:02x?}", data);
    fuzz(data, true, None);
}

fn analyze(corpus: &Path) {
    let mut stats = Stats::default();
    let mut count = 0;
    let total = std::fs::read_dir(corpus).unwrap().count();
    for entry in std::fs::read_dir(corpus).unwrap() {
        let data = std::fs::read(entry.unwrap().path()).unwrap();
        fuzz(&data, false, Some(&mut stats));
        count += 1;
        print!("\u{1b}[K{} / {}\r", count, total);
        stdout().flush().unwrap();
    }
    print!("{}", stats);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        let stdin = std::io::stdin();
        let mut data = Vec::new();
        stdin.lock().read_to_end(&mut data).unwrap();
        return debug(&data);
    }
    if args.len() > 2 {
        return usage(&args[0]);
    }
    let path = Path::new(&args[1]);
    if path.is_file() {
        debug(&std::fs::read(path).unwrap());
    } else if path.is_dir() {
        analyze(path);
    } else {
        usage(&args[0]);
    }
}
