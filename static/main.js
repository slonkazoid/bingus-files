const supportsRequestStreams = (() => {
	let duplexAccessed = false;

	const hasContentType = new Request("", {
		body: new ReadableStream(),
		method: "POST",
		get duplex() {
			duplexAccessed = true;
			return "half";
		},
	}).headers.has("Content-Type");

	return duplexAccessed && !hasContentType;
})();

const prettyFileSize = (n) => {
	if (n > 0.5 * 1000 ** 5) return (n / 1000 ** 5).toFixed(2) + " PB";
	else if (n > 0.5 * 1000 ** 4) return (n / 1000 ** 4).toFixed(2) + " TB";
	else if (n > 0.5 * 1000 ** 3) return (n / 1000 ** 3).toFixed(2) + " GB";
	else if (n > 0.5 * 1000 ** 2) return (n / 1000 ** 2).toFixed(2) + " MB";
	else if (n > 0.5 * 1000) return (n / 1000).toFixed(2) + " KB";
	else return n + " B";
};

const prettyMs = (n) => {
	let f = [];
	if (n >= 3600000) f.push(Math.floor(n / 3600000) + "h");
	if (n % 3600000 >= 60000) f.push(Math.floor((n % 3600000) / 60000) + "m");
	if (n % 60000 >= 1000) f.push(Math.floor((n % 60000) / 1000) + "s");
	if (n % 1000) f.push(Math.floor(n % 1000) + "ms");
	return f.join(" ");
};

const output = document.getElementById("output");
const form = document.querySelector("form");
const fileInput = document.querySelector("input[type=file]");
const maxUploadSizeSpan = document.getElementById("maxUploadSize");
const filesStoredSpan = document.getElementById("filesStored");
const storageUsedSpan = document.getElementById("storageUsed");
let maxFileSize = 0;

function log(str) {
	let element = document.createElement("p");
	element.innerText = str;
	output.append(element);
	return element;
}

function logElement(elem) {
	output.append(elem);
	return elem;
}

function addProgressBar() {
	let element = document.createElement("progress");
	element.max = "100";
	element.value = "0";
	output.append(element);
	return element;
}

async function updateStats() {
	let stats = await (await fetch("/stats")).json();

	maxFileSize = stats.max_file_size;

	maxUploadSizeSpan.innerText = prettyFileSize(stats.max_file_size);
	filesStoredSpan.innerText = stats.files_stored;
	storageUsedSpan.innerText = prettyFileSize(stats.storage_used);

	return stats;
}

await updateStats();

form.addEventListener("submit", (e) => {
	fileInput.dispatchEvent(new Event("change"));
});

fileInput.addEventListener("change", async (e) => {
	let start = performance.now();

	let progress = addProgressBar();
	let fileCount = fileInput.files.length;
	progress.max = fileCount;
	console.log(fileCount, fileInput.files);

	let logged = log(`uploading ${fileCount} file${fileCount == 1 ? "" : "s"}`);

	let errors = 0;

	let messages = [];

	for (let file of fileInput.files)
		messages.push(
			log(`ẁaiting to upload ${file.name} (${prettyFileSize(file.size)})`)
		);

	for (let i = 0; i < fileInput.files.length; i++) {
		let file = fileInput.files[i];

		if (file.size > maxFileSize) {
			log(
				`${file.name} is bigger than max file size (${prettyFileSize(
					file.size
				)} > ${prettyFileSize(maxFileSize)})`
			);
			continue;
		}

		let start = performance.now();
		console.log("uploading", file);
		let logged = document.createElement("p");
		logged.innerText = `uploading ${file.name} (${prettyFileSize(
			file.size
		)})`;
		messages[i].replaceWith(logged);

		let trackingStream;
		if (supportsRequestStreams) {
			let localProgress = document.createElement("progress");
			localProgress.max = file.size;
			localProgress.value = "0";
			logged.appendChild(localProgress);

			let uploaded = 0;
			trackingStream = new TransformStream({
				transform(chunk, controller) {
					controller.enqueue(chunk);
					uploaded += chunk.byteLength;
					localProgress.value = uploaded;
				},
			});
		}

		let res;
		try {
			if (supportsRequestStreams) {
				res = await fetch("/" + file.name, {
					method: "PUT",
					headers: {
						"Content-Length": file.size,
						"Content-Type": "application/octet-stream",
					},
					body: file.stream().pipeThrough(trackingStream),
					duplex: "half",
				});
			} else {
				res = await fetch("/" + file.name, {
					method: "PUT",
					body: file,
				});
			}
		} catch (err) {
			errors++;
			console.error(err);
			let p = document.createElement("p");
			p.append(`failed to upload file. see the console for more details`);
			logged.replaceWith(p);
			continue;
		}

		// TODO: better error handling
		if (res.status === 200) {
			let fileName = await res.text();
			let p = document.createElement("p");
			p.append("uploaded ");
			let a = document.createElement("a");
			a.href = `${location.origin}/${fileName}`;
			a.innerText = fileName;
			p.append(a);
			p.append(` (${prettyMs(performance.now() - start)})`);
			logged.replaceWith(p);
			progress.value = parseFloat(progress.value) + 1;
		} else {
			errors++;
			console.error(res.status, res.statusText);
			let p = document.createElement("p");
			p.append(
				`failed to upload file: ${res.statusText}. see the console for more details`
			);
			logged.replaceWith(p);
		}
	}

	let p = document.createElement("p");
	p.innerText = `uploaded ${
		fileCount - errors
	}/${fileCount} files in ${prettyMs(performance.now() - start)}`;
	logged.replaceWith(p);
	updateStats();
});

setInterval(updateStats, 15000);
