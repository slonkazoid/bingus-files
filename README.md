# bingus-files

[floppa-files](https://github.com/gosher-studios/floppa-files) competitor

# Configuration

Copy `config.default.json` to `config.json` and change the values as you please.  
You can change the config file location by changing the environment variable `CONFIG_FILE`.

Set the environment variable `DEBUG` to enable verbose output.

```js
{
	"host": "0.0.0.0", // Address to listen on
	"port": 4040, // Port to listen on
	"temp_dir": "temp", // Where to store temporary files
	"upload_dir": "files", // Where to store uploaded files (moved from temp_dir, so ideally those 2 are on the same filesystem)
	"max_upload": 1000000000 // Max file size (1 GB)
}
```

# Installation

Run `npm ci` in the project directory to download the dependencies.

# Usage

```sh
# This sets NODE_ENV to production
npm start
```
