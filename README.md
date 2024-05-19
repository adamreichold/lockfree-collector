# lockfree-collector

[![crates.io](https://img.shields.io/crates/v/lockfree-collector.svg)](https://crates.io/crates/lockfree-collector)
[![docs.rs](https://docs.rs/lockfree-collector/badge.svg)](https://docs.rs/lockfree-collector)
[![github.com](https://github.com/adamreichold/lockfree-collector/actions/workflows/test.yaml/badge.svg)](https://github.com/adamreichold/lockfree-collector/actions/workflows/test.yaml)

A lock-free collector which uses blocking to amortize the cost of heap allocations and stealing to keep the collection phase efficient, especially if there no values need to be collected.

## License

Licensed under

 * [Apache License, Version 2.0](LICENSE-APACHE) or
 * [MIT license](LICENSE-MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
