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

	let progressBar = addProgressBar();
	let fileCount = fileInput.files.length;
	progressBar.max = fileCount;
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
			errors++;
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

		if (form.progress.checked) {
			// upload with progress (sigh)

			let div = document.createElement("div");

			let localProgressBar = document.createElement("progress");
			localProgressBar.style.width = "50%";
			localProgressBar.style.display = "inline";
			localProgressBar.max = file.size;
			localProgressBar.value = "0";
			div.append(localProgressBar);

			let progressText = document.createElement("span");
			let prematureOptimization = prettyFileSize(file.size);
			progressText.innerText = `0 B/${prematureOptimization}`;
			progressText.style.float = "right";
			progressText.style["margin-left"] = "0.5em";
			progressText.title = "rate is sampled from last 15 seconds";
			div.append(progressText);

			let /** @type {XMLHttpRequest} */ res;
			try {
				// i love callbacks
				res = await new Promise((resolve, reject) => {
					/**
					 * @typedef {Object} rateSnapshot
					 * @property {number} ts
					 * @property {number} total
					 */
					let /** @type {rateSnapshot[]} */ rateCalcMagicArray = [];

					let /** @type {number} */ uploadStart;

					let req = new XMLHttpRequest();
					req.open("PUT", "/" + file.name);
					req.upload.addEventListener("progress", (e) => {
						console.log("progress event", e);
						localProgressBar.value = e.loaded;
						let ratio = e.loaded / file.size;
						let now = Date.now();
						rateCalcMagicArray.push({
							ts: now,
							total: e.loaded,
						});
						let firstSnapshot = rateCalcMagicArray.filter(
							(snapshot) => snapshot.ts + 15 * 1000 >= now
						)[0];
						let rate =
							firstSnapshot === undefined
								? 0
								: (e.loaded - firstSnapshot.total) /
								  (now - firstSnapshot.ts);

						progressText.innerText = `${prettyFileSize(
							e.loaded
						)}/${prematureOptimization} (${(ratio * 100).toFixed(
							2
						)}%) ${prettyFileSize(rate * 1000)}/s`;
					});
					req.addEventListener("loadstart", () => {
						uploadStart = Date.now();
						rateCalcMagicArray.push({
							ts: uploadStart,
							total: 0,
						});
						logged.appendChild(div);
					});
					req.addEventListener("error", () => reject(req));
					req.addEventListener("abort", () => reject(req));
					req.addEventListener("load", () => resolve(req));
					req.addEventListener("loadend", () => div.remove());
					req.send(file);
				});
			} catch (err) {
				errors++;
				p.append(
					`failed to upload file. see the console for more details`
				);
				console.error(err);
				continue;
			}

			if (res.status === 200) {
				let fileName = res.responseText;
				let p = document.createElement("p");
				p.append("uploaded ");
				let a = document.createElement("a");
				a.href = `${location.origin}/${fileName}`;
				a.innerText = fileName;
				p.append(a);
				p.append(` (${prettyMs(performance.now() - start)})`);
				logged.replaceWith(p);
				progressBar.value = parseFloat(progressBar.value) + 1;
			} else {
				errors++;
				console.error(res.status, res.statusText);
				let p = document.createElement("p");
				p.append(
					`failed to upload file: ${res.statusText}. see the console for more details`
				);
				logged.replaceWith(p);
			}
		} else {
			// upload without torturing self

			let /** @type {Response} */ res;
			try {
				res = await fetch("/" + file.name, {
					method: "PUT",
					body: file,
				});
			} catch (err) {
				errors++;
				console.error(err);
				let p = document.createElement("p");
				p.append(
					`failed to upload file. see the console for more details`
				);
				logged.replaceWith(p);
				continue;
			}

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
				progressBar.value = parseFloat(progressBar.value) + 1;
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
	}

	let p = document.createElement("p");
	p.innerText = `uploaded ${
		fileCount - errors
	}/${fileCount} files in ${prettyMs(performance.now() - start)}`;
	logged.replaceWith(p);
	progressBar.remove();
	updateStats();
});

setInterval(updateStats, 15000);
