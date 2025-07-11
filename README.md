# cargo-pumpkin

A Cargo subcommand that building and running your Pumpkin plugin.

## Installation

```bash
cargo install cargo-pumpkin
```

## Usage

### Initialize
```bash
cargo pumpkin init
```

### Build and run
```bash
cargo pumpkin
# or explicitly
cargo pumpkin run
```

### Options
```bash
# Force rebuild of Pumpkin even if it exists
cargo pumpkin run --force

# Skip building the current project
cargo pumpkin run --skip-self-build

# Clean the .run directory
cargo pumpkin clean
```