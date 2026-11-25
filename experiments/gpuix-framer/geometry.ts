export type Rect = { x: number; y: number; width: number; height: number };
export type Point = { x: number; y: number };

export const clamp01 = (value: number) => Math.min(1, Math.max(0, value));

export function toFocal(photo: Rect, windowX: number, windowY: number): Point {
	return {
		x: photo.width > 0 ? clamp01((windowX - photo.x) / photo.width) : 0.5,
		y: photo.height > 0 ? clamp01((windowY - photo.y) / photo.height) : 0.5
	};
}

export function crop(focal: Point, imageWidth: number, imageHeight: number, side: number): Rect {
	const x = Math.min(imageWidth - side, Math.max(0, focal.x * imageWidth - side / 2));
	const y = Math.min(imageHeight - side, Math.max(0, focal.y * imageHeight - side / 2));
	return { x, y, width: side, height: side };
}
