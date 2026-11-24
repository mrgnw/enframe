export type Rect = { x: number; y: number; width: number; height: number };
export type Point = { x: number; y: number };

export const clamp01 = (value: number) => Math.min(1, Math.max(0, value));

export function fit(area: Rect, imageWidth: number, imageHeight: number): Rect {
	const scale = Math.min(area.width / imageWidth, area.height / imageHeight);
	const width = imageWidth * scale;
	const height = imageHeight * scale;
	return { x: area.x + (area.width - width) / 2, y: area.y + (area.height - height) / 2, width, height };
}

export function toFocal(photo: Rect, windowX: number, windowY: number): Point {
	return { x: clamp01((windowX - photo.x) / photo.width), y: clamp01((windowY - photo.y) / photo.height) };
}

export function crop(focal: Point, imageWidth: number, imageHeight: number, side: number): Rect {
	const x = Math.min(imageWidth - side, Math.max(0, focal.x * imageWidth - side / 2));
	const y = Math.min(imageHeight - side, Math.max(0, focal.y * imageHeight - side / 2));
	return { x, y, width: side, height: side };
}
