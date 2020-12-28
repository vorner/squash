# Squash

[![Actions Status](https://github.com/vorner/squash/workflows/test/badge.svg)](https://github.com/vorner/squash/actions)
[![codecov](https://codecov.io/gh/vorner/squash/branch/main/graph/badge.svg?token=0SVW5CJLZQ)](https://codecov.io/gh/vorner/squash)
[![docs](https://docs.rs/squash/badge.svg)](https://docs.rs/squash)

More space-efficient way to encode owned slices and string slices on the heap
(single pointer on the stack + down to one byte to encode the length on the
heap).

Further details are in the [the documentation](https://docs.rs/squash).

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms
or conditions.
