/**
 * @typedef {Object} RateSnapshot snapshot of upload progress
 * @property {number} ts when the snapshot was taken, `performance.now`
 * @property {number} total total uploaded bytes at the time of snapshot
 */

export default class RateCalc {
	/**
	 * @param {Number} [sampleSize=15000] sample size in milliseconds
	 */
	constructor(sampleSize = 15000) {
		/** @type {RateSnapshot[]} */
		this.snapshots = [];
		this.sampleSize = sampleSize;
		this.total = 0;
		this.snapshot({
			ts: performance.now(),
			total: 0,
		});
	}

	/**
	 * @param {RateSnapshot} s
	 */
	snapshot(s) {
		let now = performance.now();
		while (this.snapshots[0]?.ts + this.sampleSize < now)
			this.snapshots.shift();
		this.snapshots.push(s);
		this.total = s.total;
	}

	/**
	 * @returns {Number} bytes per second
	 */
	rate(sampleSize = 15000) {
		let now = performance.now();
		let firstSnapshot = this.snapshots.find(
			(snapshot) => snapshot.ts + sampleSize >= now
		);
		if (firstSnapshot === undefined) return 0;

		let passedTime = now - firstSnapshot.ts;
		let bytes = this.total - firstSnapshot.total;
		return (bytes / passedTime) * 1000;
	}
}
