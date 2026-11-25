import { readFileSync } from 'node:fs';
import { inflateSync } from 'node:zlib';

export type Pixels = { width: number; height: number; data: Buffer };

export function readPng(path: string): Pixels {
	const file = readFileSync(path);
	const width = file.readUInt32BE(16);
	const height = file.readUInt32BE(20);
	assert8BitRgba(file[24], file[25], file[28]);
	const chunks: Buffer[] = [];
	for (let at = 8; at < file.length; ) {
		const length = file.readUInt32BE(at);
		if (file.toString('latin1', at + 4, at + 8) === 'IDAT') chunks.push(file.subarray(at + 8, at + 8 + length));
		at += length + 12;
	}
	const raw = inflateSync(Buffer.concat(chunks));
	const stride = width * 4;
	const data = Buffer.alloc(stride * height);
	for (let y = 0; y < height; y++) {
		const filter = raw[y * (stride + 1)];
		for (let i = 0; i < stride; i++) {
			const left = i >= 4 ? data[y * stride + i - 4] : 0;
			const up = y > 0 ? data[(y - 1) * stride + i] : 0;
			const upLeft = y > 0 && i >= 4 ? data[(y - 1) * stride + i - 4] : 0;
			const p = left + up - upLeft;
			const pa = Math.abs(p - left);
			const pb = Math.abs(p - up);
			const pc = Math.abs(p - upLeft);
			const paeth = pa <= pb && pa <= pc ? left : pb <= pc ? up : upLeft;
			const predictor = [0, left, up, (left + up) >> 1, paeth][filter];
			data[y * stride + i] = (raw[y * (stride + 1) + 1 + i] + predictor) & 255;
		}
	}
	return { width, height, data };
}

function assert8BitRgba(depth: number, colorType: number, interlace: number) {
	if (depth !== 8 || colorType !== 6 || interlace !== 0) throw new Error(`unsupported PNG ${depth}/${colorType}/${interlace}`);
}

export type Box = { x: number; y: number; width: number; height: number };

export function region(image: Pixels, box: Box, scale: number): Buffer {
	const x0 = Math.round(box.x * scale);
	const y0 = Math.round(box.y * scale);
	const w = Math.round(box.width * scale);
	const h = Math.round(box.height * scale);
	const rows: Buffer[] = [];
	for (let y = y0; y < y0 + h; y++) rows.push(image.data.subarray((y * image.width + x0) * 4, (y * image.width + x0 + w) * 4));
	return Buffer.concat(rows);
}
