const output = document.getElementById("output");
const form = document.querySelector("form");
const fileInput = document.querySelector("input[type=file]");

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

form.addEventListener("submit", (e) => {
	fileInput.dispatchEvent(new Event("change"));
});

fileInput.addEventListener("change", (e) => {
	const file = fileInput.files[0];
	if (!file) return;

	let start = performance.now();
	log(`uploading ${file.name}`);
	let progress = addProgressBar();

	fetch("/" + file.name, {
		method: "PUT",
		body: file,
	})
		.then((res) => {
			progress.value = "90";
			return res.text();
		})
		.then((fileName) => {
			let p = document.createElement("p");
			p.append("uploaded ");
			let a = document.createElement("a");
			a.href = `${location.origin}/file/${fileName}`;
			a.innerText = fileName;
			p.append(a);
			p.append(` (${(performance.now() - start).toFixed(2)}ms)`);
			progress.replaceWith(p);
		})
		.catch((error) => {
			console.error(error);
			let p = document.createElement("p");
			p.append("failed to upload file. see the console for more details");
			progress.replaceWith(p);
		});
});
