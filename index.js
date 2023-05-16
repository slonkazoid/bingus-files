const { info, warn, error, debug } = require("./logger");
const fs = require("fs");
const path = require("path");
const color = require("ansi-colors");
const express = require("express");
const fileupload = require("express-fileupload");

const filename_prefix = () =>
	Math.floor(Math.random() * 0x100000000).toString(36);
const prettyFileSize = (n) => {
	if (n > 0.5 * 1000 ** 5) return (n / 1000 ** 5).toFixed(2) + " PB";
	else if (n > 0.5 * 1000 ** 4) return (n / 1000 ** 4).toFixed(2) + " TB";
	else if (n > 0.5 * 1000 ** 3) return (n / 1000 ** 3).toFixed(2) + " GB";
	else if (n > 0.5 * 1000 ** 2) return (n / 1000 ** 2).toFixed(2) + " MB";
	else if (n > 0.5 * 1000) return (n / 1000).toFixed(2) + " KB";
	else return n + " B";
};
const checkDir = (dir) => {
	debug(`Checking if ${color.bold(dir)} exists and is a directory`);

	if (fs.existsSync(dir)) {
		debug("Exists");
		let filesStat = fs.statSync(dir);
		if (!filesStat.isDirectory()) {
			error(`${color.bold(dir)} exists but is not a directory!`);
			process.exit(1);
		}
	} else {
		debug("Doesn't exist, creating");
		fs.mkdirSync(dir);
	}
};

function readDir() {
	let dir = fs.readdirSync(config.upload_dir);
	let size = 0;
	for (let file of dir)
		size += fs.statSync(path.join(config.upload_dir, file)).size;
	return { size, length: dir.length, files: dir };
}

const CONFIG_FILE = process.env.CONFIG_FILE ?? "config.json";
const config = {
	// Defaults
	host: "0.0.0.0",
	port: 4040,
	temp_dir: "temp",
	upload_dir: "files",
	max_upload: 1000 ** 3, // 1 GB
};

info("Starting up...");

const parseConfig = () => {
	debug("Loading configuration");
	let configLoadStartTime = performance.now();

	try {
		let _config = JSON.parse(fs.readFileSync(CONFIG_FILE));
		for (let key in _config) {
			if (config.hasOwnProperty(key)) {
				if (key === "upload_dir" || key === "temp_dir")
					checkDir(_config[key]);
				config[key] = _config[key];
			} else
				debug(
					`Found unknown option ${color.bold(
						key
					)} in config file, ignoring.`
				);
		}
	} catch (e) {
		warn(`There was an error parsing the configuration file.`);
		console.error(e);
	}

	debug(
		`Loaded configuration in ${color.bold(
			(performance.now() - configLoadStartTime).toFixed(2) + "µs"
		)}`
	);
};

if (fs.existsSync(CONFIG_FILE)) {
	parseConfig();
	fs.watchFile(CONFIG_FILE, () => {
		debug("Configuration file updated, reloading...");
		parseConfig();
	});
} else warn("Configuration file does not exist, using default options.");

debug("Initializing express app");

let appStartTime = performance.now();
let { size: currentSize, length: currentFiles } = readDir();

const app = express();

app.set("view engine", "ejs");
app.set("views", "views");

app.use((req, res, next) => {
	let requestReceiveTime = performance.now();
	debug(`Received ${req.method} request ${req.path} by ${req.ip}`);
	res.on("close", () => {
		info(
			`${(
				[
					,
					,
					color.black.bgGreenBright,
					color.black.bgYellowBright,
					color.white.bgRed,
					color.white.bgRed,
				][Math.floor(res.statusCode / 100)] ?? color.white.bgBlack
			)(res.statusCode)} ${req.method} ${req.path} by ${
				req.ip
			} done in ${color.bold(
				(performance.now() - requestReceiveTime).toFixed(2) + "ms"
			)}`
		);
	});
	next();
});
app.use(express.static("static"));
app.use(
	express.static(config.upload_dir, {
		index: false,
	})
);

app.get("/", (req, res) => {
	res.render("index", {
		max_upload: config.max_upload,
		max_upload_pretty: prettyFileSize(config.max_upload),
		host: req.protocol + "://" + req.headers.host,
	});
});
app.get("/stats", (req, res) => {
	res.send({
		storage_used: currentSize,
		file_count: currentFiles,
		uptime: Math.abs(Date.now() - appListenTime) / 1000,
		max_upload: config.max_upload,
	});
});

app.post(
	"/up",
	fileupload({
		limits: { files: 1, fileSize: config.max_upload },
		useTempFiles: true,
		tempFileDir: config.temp_dir,
	}),
	async (req, res) => {
		if (!req.files?.["file"]) {
			res.sendStatus(400);
			return;
		}
		let filename = `${filename_prefix()}.${req.files["file"].name}`;
		info(
			`${req.ip} uploaded ${color.bold(filename)} (${color.bold(
				prettyFileSize(req.files["file"].size)
			)})`
		);
		req.files["file"].mv(path.join(config.upload_dir, filename));
		currentSize += req.files["file"].size;
		currentFiles++;
		res.send(filename);
	}
);

// Seconds
let appListenTime = 0;
app.listen(config.port, config.host, () => {
	appListenTime = Date.now();
	info(
		`Listening on http://${color.bold(config.host)}:${color.bold(
			config.port
		)}/`
	);
});

setInterval(
	() => ({ size: currentSize, length: currentFiles } = readDir()),
	30000
);

debug(
	`Initialized express app ${color.bold(
		(performance.now() - appStartTime).toFixed(2) + "µs"
	)}`
);
