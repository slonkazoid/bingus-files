/**
 * @typedef {Object} rateSnapshot snapshot of upload progress
 * @property {number} ts when the snapshot was taken, `performance.now`
 * @property {number} total total uploaded bytes at the time of snapshot
 */

// constants
const output = document.getElementById("output");
const form = document.querySelector("form");
const fileInput = document.querySelector("input[type=file]");
const maxUploadSizeSpan = document.getElementById("max-upload-size");
const filesStoredSpan = document.getElementById("files-stored");
const storageUsedSpan = document.getElementById("storage-used");
let maxFileSize = 0;
let sampleSize = 15 * 1000;

window.setSampleSize = (n) => (sampleSize = n);
console.log("hey, you can change the upload rate sample size (milliseconds)");
console.log(
	"try %csetSampleSize(n)",
	`
	padding: 0.25em 0.5em;
	background-color: #1e1e2e;
	color: #cdd6f4;
`
);

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

function log(str) {
	let element = document.createElement("p");
	element.innerText = str;
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

/* @param {FileList} files */
async function uploadFiles(files) {
	let start = performance.now();

	let totalSize = 0;
	for (let file of files) {
		totalSize += file.size;
	}
	let fileCount = files.length;
	console.log(fileCount, prettyFileSize(totalSize), files);

	let /** @type {rateSnapshot[]} */ rateCalcMagicArray = [];
	let progressDiv = document.createElement("div");

	let progressBar = document.createElement("progress");
	progressBar.style.width = "50%";
	progressBar.style.display = "inline";
	progressBar.max = totalSize;
	progressBar.value = "0";
	progressDiv.append(progressBar);

	let progressText = document.createElement("span");
	progressText.innerText = `0 B/${prettyFileSize(totalSize)}`;
	progressText.style.float = "right";
	progressText.style["margin-left"] = "0.5em";
	progressText.title = `rate is sampled from last ${prettyMs(sampleSize)}`;
	progressDiv.append(progressText);

	let placeholder = document.getElementById("output-placeholder");
	if (placeholder instanceof Element) placeholder.replaceWith(progressDiv);
	else {
		output.append(document.createElement("hr"));
		output.append(progressDiv);
	}

	let logged = log(
		`uploading ${fileCount} file${
			fileCount == 1 ? "" : "s"
		} (${prettyFileSize(totalSize)} total)`
	);

	let links = [];

	let messages = [];

	for (let file of files)
		messages.push(
			log(`ẁaiting to upload ${file.name} (${prettyFileSize(file.size)})`)
		);

	for (let i = 0; i < fileCount; i++) {
		let file = files[i];

		if (file.size > maxFileSize) {
			messages[i].innerText = `${
				file.name
			} is bigger than max file size (${prettyFileSize(
				file.size
			)} > ${prettyFileSize(maxFileSize)})`;
			continue;
		}

		let start = performance.now();
		console.log("uploading", file);
		let logged = document.createElement("p");
		logged.innerText = `uploading ${file.name} (${prettyFileSize(
			file.size
		)})`;
		messages[i].replaceWith(logged);

		// upload with progress (sigh)

		let localDiv = document.createElement("div");

		let localProgressBar = document.createElement("progress");
		localProgressBar.style.width = "50%";
		localProgressBar.style.display = "inline";
		localProgressBar.max = file.size;
		localProgressBar.value = "0";
		localDiv.append(localProgressBar);

		let localProgressText = document.createElement("span");
		localProgressText.innerText = `0 B/${prettyFileSize(file.size)}`;
		localProgressText.style.float = "right";
		localProgressText.style["margin-left"] = "0.5em";
		localProgressText.title = "rate is sampled from last 15 seconds";
		localDiv.append(localProgressText);

		let /** @type {XMLHttpRequest} */ res;
		try {
			// i love callbacks
			res = await new Promise((resolve, reject) => {
				let /** @type {rateSnapshot[]} */ localRateCalcMagicArray = [];
				let /** @type {number} */ localUploadStart;

				let startBytes = parseInt(progressBar.value);
				let rateCalcMagicArrayStartIndex = rateCalcMagicArray.length;

				let req = new XMLHttpRequest();
				req.open("PUT", new URL(file.name, location.origin));
				req.upload.addEventListener("progress", (e) => {
					console.log("progress event", e);
					localProgressBar.value = e.loaded;
					progressBar.value = startBytes + e.loaded;
					let localRatio = e.loaded / file.size;
					let ratio = (startBytes + e.loaded) / totalSize;
					let now = performance.now();

					// local is for current file,
					// the regular one is for the current upload group
					let localSnapshot = {
						ts: now,
						total: e.loaded,
					};
					let snapshot = {
						ts: now,
						total: startBytes + e.loaded,
					};

					localRateCalcMagicArray.push(localSnapshot);
					rateCalcMagicArray.push(snapshot);

					let localFirstSnapshot = rateCalcMagicArray.find(
						(snapshot, index) =>
							index >= rateCalcMagicArrayStartIndex &&
							snapshot.ts + sampleSize >= now
					);
					let firstSnapshot = rateCalcMagicArray.find(
						(snapshot) => snapshot.ts + sampleSize >= now
					);

					// the rate is in bytes/millisecond
					let localRate =
						localFirstSnapshot === undefined
							? 0
							: (e.loaded - localFirstSnapshot.total) /
							  (now - localFirstSnapshot.ts);
					let rate =
						firstSnapshot === undefined
							? 0
							: (startBytes + e.loaded - firstSnapshot.total) /
							  (now - firstSnapshot.ts);

					localProgressText.innerText = `${prettyFileSize(
						e.loaded
					)}/${prettyFileSize(file.size)} (${(
						localRatio * 100
					).toFixed(2)}%) ${prettyFileSize(localRate * 1000)}/s`;
					progressText.innerText = `${prettyFileSize(
						startBytes + e.loaded
					)}/${prettyFileSize(totalSize)} (${(ratio * 100).toFixed(
						2
					)}%) ${prettyFileSize(rate * 1000)}/s`;
				});
				req.addEventListener("loadstart", () => {
					localUploadStart = performance.now();
					rateCalcMagicArray.push({
						ts: localUploadStart,
						total: 0,
					});
					logged.appendChild(localDiv);
				});
				req.addEventListener("error", () => reject(req));
				req.addEventListener("abort", () => reject(req));
				req.addEventListener("load", () => resolve(req));
				req.addEventListener("loadend", () => localDiv.remove());
				req.send(file);
			});
		} catch (err) {
			let p = document.createElement("p");
			p.append(`failed to upload file. see the console for more details`);
			logged.replaceWith(p);
			console.error(err);
			continue;
		}

		if (res.status === 200) {
			let fileName = res.responseText;
			let p = document.createElement("p");
			p.append("uploaded ");
			let a = document.createElement("a");
			let link = new URL(fileName, location.origin);
			a.href = link;
			links.push(link);
			a.innerText = fileName;
			p.append(a);
			p.append(` (${prettyMs(performance.now() - start)})`);
			logged.replaceWith(p);
			progressBar.value = parseFloat(progressBar.value) + 1;
			updateStats();
		} else {
			console.error(res.status, res.statusText);
			let p = document.createElement("p");
			p.append(
				`failed to upload file: ${res.statusText}. see the console for more details`
			);
			logged.replaceWith(p);
		}
	}

	let time = performance.now() - start;
	let firstSnapshot = rateCalcMagicArray[0];
	let totalUploaded = 0;
	if (typeof firstSnapshot === "object") {
		let lastSnapshot = rateCalcMagicArray.pop();
		totalUploaded = lastSnapshot.total;
	}
	let rate = totalUploaded / time;

	let p = document.createElement("p");
	p.innerText = `uploaded ${
		links.length
	}/${fileCount} files (${prettyFileSize(totalUploaded)}/${prettyFileSize(
		totalSize
	)}) in ${prettyMs(time)} (${prettyFileSize(rate * 1000)}/s)`;

	if (links.length > 0) {
		let button = document.createElement("button");
		button.innerText = links.length > 1 ? "copy all" : "copy";
		button.onclick = () => {
			navigator.clipboard.writeText(links.join("\n "));
			button.style.setProperty("border-color", "var(--mauve)");
		};
		p.append(document.createTextNode(" "));
		p.append(button);
	}

	logged.replaceWith(p);
	progressDiv.remove();
	updateStats();
}

form.addEventListener("submit", (e) => {
	fileInput.dispatchEvent(new Event("change"));
	e.preventDefault();
});

fileInput.addEventListener("change", () => uploadFiles(fileInput.files));

document.addEventListener("drop", (e) => {
	e.preventDefault();

	uploadFiles(
		e.dataTransfer.items === undefined
			? e.dataTransfer.files
			: [...e.dataTransfer.items]
					.filter((item) => item.kind === "file")
					.map((item) => item.getAsFile())
	);
});

document.addEventListener("dragover", (e) => e.preventDefault());

setInterval(updateStats, 60000);
