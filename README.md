# bingus-files

Simple file sharing service written in Rust using [axum](https://github.com/tokio-rs/axum).

Also see [floppa-files](https://github.com/gosher-studios/floppa-files).

## Features

- Upload files
- See number of uploaded files, along with how much space they take
- Download/view files

## Configuration

Configuration files are read in this order:

1. `$BINGUS_CONFIG`
2. `config.toml`
3. `$XDG_CONFIG_HOME/bingus-files/config.toml`  
   `%APPDATA%\bingus-files\config.toml` on windows
4. `/etc/bingus-files/config.toml` (not on windows)

### Example configuration file (all defaults)

```toml
upload_dir="files"        # where to store uploaded files
temp_dir="temp"           # unused at the moment
prefix_length=8           # controls the length of the random prefix prepended to
                          # file names with a '.', set to 0 to disable
max_file_size=1000000000  # self explanatory (1 GB)
max_file_name_length=200  # self explanatory
stats_interval=60         # how many seconds to wait between stats refreshes,
                          # set to 0 to disable
fallocate=true            # use the `fallocate(2)` syscall (linux only, requires
                          # feature `fallocate`)

[http]
host="0.0.0.0"            # host to listen on
port=4040                 # port to listen on
concurrency_limit=512     # max number of threads to launch for request handling,
                          # set to 0 for unlimited
behind_proxy=false        # trust the X-Forwarded-For header

[db]                      # database options (not implemented)
enable=false              # enable database
```

## `fallocate(2)` support

bingus-files uses `fallocate(2)` to preallocate space for uploads.  
You can turn this feature off by passing `--no-default-features` to `cargo`.  
This feature is only implemented for Linux targets.

## Todo

- Compression
- Better progress tracking for frontend
- File expiration
- Paste functionality
- More stats

### Non-goals

- HTTPS (use a reverse proxy if you want HTTPS)
- Accounts
- Tracking (except for the bare minimum to stay legal)
