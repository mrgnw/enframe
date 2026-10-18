import { carveSeams, type SeamRequest } from './seams';

export type SeamReply = { step: number; seam: Int32Array; elapsed: number } | { done: true; elapsed: number };

declare function postMessage(message: SeamReply, transfer?: Transferable[]): void;

self.onmessage = ({ data }: MessageEvent<SeamRequest>) => {
	const started = performance.now();
	carveSeams(data, (step, seam) => postMessage({ step, seam, elapsed: performance.now() - started }, [seam.buffer]));
	postMessage({ done: true, elapsed: performance.now() - started });
};
