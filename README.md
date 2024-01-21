# bingus-files

Simple file sharing service written in Rust using [axum](https://github.com/tokio-rs/axum).

Also see [floppa-files](https://github.com/gosher-studios/floppa-files).

**This project is now in maintenance-only mode.**

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
allocate=true             # preallocate space for uploads with Content-Length

[http]
host="0.0.0.0"            # host to listen on
port=4040                 # port to listen on
concurrency_limit=512     # max number of threads to launch for request handling,
                          # set to 0 for unlimited
behind_proxy=false        # trust the X-Forwarded-For header

[logging]
level="info"              # "error", "warn", "info", "debug", "trace"
stderr=true               # enables logging to stderr
file=false                # enables logging to file (setting this to a string
                          # will also enable it, and set the output path)
                          # this supports chrono date formatting
                          # default path is bingus-files_%Y-%m-%dT%H:%M:%S%:z.log
```

## Todo

[x] Compression
[x] Better progress tracking for frontend
[ ] File expiration
[ ] Paste functionality
[ ] More stats

### Non-goals

- HTTPS (use a reverse proxy if you want HTTPS)
- Accounts
- Tracking (except for the bare minimum to stay legal)
