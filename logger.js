const color = require("ansi-colors");

const getTime = () =>
	color.blue.bold(`[${color.white(new Date().toLocaleTimeString())}]`);

module.exports = {
	info() {
		console.log(getTime(), color.black.bgWhiteBright("INFO"), ...arguments);
	},
	warn() {
		console.error(
			getTime(),
			color.black.bgYellowBright("WARN"),
			...arguments
		);
	},
	error() {
		console.error(
			getTime(),
			color.black.bgRedBright("ERROR"),
			...arguments
		);
	},
	debug() {
		if (process.env.DEBUG) return;
		console.log(
			getTime(),
			color.black.bgWhiteBright("DEBUG"),
			...arguments
		);
	},
};
