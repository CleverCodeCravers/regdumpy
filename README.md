# regdumpy

`regdumpy` is a small Rust command-line tool to export the Windows registry to a JSON-lines file. Each line represents one registry value so the dump can be diffed easily with standard tools.

## Features

* Dump starting from any root hive (e.g. `HKEY_LOCAL_MACHINE`).
* Detects incorrect registry data and marks it instead of crashing.
* Warns when the program is not executed with administrative privileges.
* JSONL output – great for incremental parsing and diffing.

## Installation

```bash
cargo install --path .
```

(or download a binary from the releases).

## Usage

```bash
regdumpy --output dump.jsonl --root HKEY_LOCAL_MACHINE
```

Options:

| Flag | Description | Default |
|------|-------------|---------|
| `-o`, `--output <FILE>` | Output file path (required) | – |
| `-r`, `--root <ROOT>`   | Root hive to start dumping | `HKEY_CURRENT_USER` |

## Example

```bash
regdumpy -o before.jsonl
# … perform some system changes …
regdumpy -o after.jsonl

# diff using jq for pretty output
jq -S . before.jsonl > before_sorted.jsonl
jq -S . after.jsonl  > after_sorted.jsonl
diff before_sorted.jsonl after_sorted.jsonl
```

## Development

```bash
# run unit tests
cargo test

# build release binary
cargo build --release
```

## License

MIT
