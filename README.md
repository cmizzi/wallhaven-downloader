# Wallhaven downloader

A simple CLI tool to download Wallhaven wallpapers directly from your terminal.

> Work in progress

## Installation

```
git clone git@github.com:cmizzi/wallhaven-downloader.git
cd wallhaven-downloader
cargo build --all --release

# Execute the binary. You can copy/move it into your $PATH.
./target/release/wallhaven-downloader --help
```

## Usage

```
wallhaven-downloader --limit 20 1920x1080 ~/Pictures/Wallpapers
```

### Advanced usage

```
> wallhaven-downloader --help

USAGE:
    wallhaven-downloader [FLAGS] [OPTIONS] <resolutions> <output>

ARGS:
    <resolutions>    Resolution is exact. The format should match the following pattern: <width>x<height>
    <output>         Directory to store wallpapers

FLAGS:
    -h, --help       Prints help information
    -v, --verbose    Configure verbosity
    -V, --version    Prints version information

OPTIONS:
    -c, --categories <categories>    Based on the following format : [General, Anime, People] [default: 111]
    -d, --direction <direction>      Sort direction [default: desc]
    -l, --limit <limit>              Limit the number of wallpapers to download [default: 10]
    -p, --purity <purity>            Based on the following format : [SFW, Sketchy] [default: 100]
    -s, --sorting <sorting>          Default sort to apply [default: random]
```
