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
	if (n > 3600000) f.push(Math.floor((n % 3600000) / 3600000) + "h");
	if (n > 60000) f.push(Math.floor((n % 60000) / 60000) + "m");
	if (n > 1000) f.push(Math.floor((n % 1000) / 1000) + "s");
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

fileInput.addEventListener("change", (e) => {
	let progress = addProgressBar();
	let fileCount = fileInput.files.length;
	progress.max = fileCount;
	console.log(fileCount, fileInput.files);

	let logged = log(`uploading ${fileCount} file${fileCount == 1 ? "" : "s"}`);

	let promises = [];

	let start = performance.now();

	for (let file of fileInput.files) {
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
		let logged = log(`uploading ${file.name}`);

		let p = fetch("/" + file.name, {
			method: "PUT",
			body: file,
		})
			.then((r) => r.text())
			.then((fileName) => {
				let p = document.createElement("p");
				p.append("uploaded ");
				let a = document.createElement("a");
				a.href = `${location.origin}/file/${fileName}`;
				a.innerText = fileName;
				p.append(a);
				p.append(` (${prettyMs(performance.now() - start)})`);
				logged.replaceWith(p);
				progress.value = parseFloat(progress.value) + 1;
			})
			.catch((error) => {
				console.error(error);
				let p = document.createElement("p");
				p.append(
					"failed to upload file. see the console for more details"
				);
				logged.replaceWith(p);
			});
		promises.push(p);
	}

	Promise.all(promises).then(() => {
		let p = document.createElement("p");
		p.innerText = `uploaded ${fileCount} files in ${prettyMs(
			performance.now() - start
		)}`;
		logged.replaceWith(p);
		updateStats();
	});
});

setInterval(updateStats, 15000);
