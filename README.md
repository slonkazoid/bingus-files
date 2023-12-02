# bingus-files

[floppa-files](https://github.com/gosher-studios/floppa-files) competitor

rewritten with axum

# Configuration

configuration is read in this order:

1. `$BINGUS_CONFIG`
2. `config.toml`
3. `$XDG_CONFIG_HOME/bingus-files/config.toml`
4. `/etc/bingus-files/config.toml` (not on windows)

```toml
host="0.0.0.0"
port=4040
upload_dir=files
temp_dir=temp
prefix_length=8
max_file_size=1_000_000_000
max_file_name_length=100
stats_interval=60
behind_proxy=false
```

set behind_proxy=true to trust the server to send the correct ip in X-Forwarded-For
