import Database from '@tauri-apps/plugin-sql';

let dbPromise: Promise<Database> | null = null;

export function getDb(): Promise<Database> {
	if (!dbPromise) {
		dbPromise = Database.load('sqlite:enframe.db');
	}
	return dbPromise;
}

export interface Photo {
	id: number;
	file_path: string;
	width?: number;
	height?: number;
	title?: string;
	description?: string;
	lr_asset_id?: string;
	created_at: string;
}

export interface Frame {
	id: number;
	photo_id: number;
	label?: string;
	x: number;
	y: number;
	width: number;
	height: number;
	rotation: number;
	is_default: boolean;
	created_at: string;
}

export interface FocalPoint {
	id: number;
	photo_id: number;
	frame_id?: number;
	x: number;
	y: number;
	label?: string;
	required: boolean;
	created_at: string;
}

export async function listPhotos(): Promise<Photo[]> {
	const db = await getDb();
	return db.select<Photo[]>('SELECT * FROM photos ORDER BY created_at DESC');
}

export async function getPhoto(id: number): Promise<Photo | null> {
	const db = await getDb();
	const results = await db.select<Photo[]>('SELECT * FROM photos WHERE id = $1', [id]);
	return results[0] || null;
}

export async function addPhoto(photo: Omit<Photo, 'id' | 'created_at'>): Promise<number> {
	const db = await getDb();
	const result = await db.execute(
		'INSERT INTO photos (file_path, width, height, title, description, lr_asset_id) VALUES ($1, $2, $3, $4, $5, $6)',
		[photo.file_path, photo.width, photo.height, photo.title, photo.description, photo.lr_asset_id]
	);
	return result.lastInsertId ?? 0;
}

export async function getFrames(photoId: number): Promise<Frame[]> {
	const db = await getDb();
	return db.select<Frame[]>('SELECT * FROM frames WHERE photo_id = $1', [photoId]);
}

export async function listAllFrames(): Promise<Frame[]> {
	const db = await getDb();
	return db.select<Frame[]>('SELECT * FROM frames');
}

export async function addFrame(frame: Omit<Frame, 'id' | 'created_at'>): Promise<number> {
	const db = await getDb();
	const result = await db.execute(
		'INSERT INTO frames (photo_id, label, x, y, width, height, rotation, is_default) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)',
		[
			frame.photo_id,
			frame.label,
			frame.x,
			frame.y,
			frame.width,
			frame.height,
			frame.rotation ?? 0,
			frame.is_default ? 1 : 0
		]
	);
	return result.lastInsertId ?? 0;
}

export async function updateFrame(
	id: number,
	updates: Partial<Omit<Frame, 'id' | 'photo_id' | 'created_at'>>
): Promise<void> {
	const db = await getDb();
	const sets: string[] = [];
	const values: unknown[] = [];

	if (updates.label !== undefined) {
		sets.push(`label = $${sets.length + 1}`);
		values.push(updates.label);
	}
	if (updates.x !== undefined) {
		sets.push(`x = $${sets.length + 1}`);
		values.push(updates.x);
	}
	if (updates.y !== undefined) {
		sets.push(`y = $${sets.length + 1}`);
		values.push(updates.y);
	}
	if (updates.width !== undefined) {
		sets.push(`width = $${sets.length + 1}`);
		values.push(updates.width);
	}
	if (updates.height !== undefined) {
		sets.push(`height = $${sets.length + 1}`);
		values.push(updates.height);
	}
	if (updates.rotation !== undefined) {
		sets.push(`rotation = $${sets.length + 1}`);
		values.push(updates.rotation);
	}

	if (sets.length === 0) return;

	values.push(id);
	await db.execute(`UPDATE frames SET ${sets.join(', ')} WHERE id = $${values.length}`, values);
}

export async function getFocalPoints(photoId: number): Promise<FocalPoint[]> {
	const db = await getDb();
	return db.select<FocalPoint[]>('SELECT * FROM focal_points WHERE photo_id = $1', [photoId]);
}

export async function addFocalPoint(point: Omit<FocalPoint, 'id' | 'created_at'>): Promise<number> {
	const db = await getDb();
	const result = await db.execute(
		'INSERT INTO focal_points (photo_id, frame_id, x, y, label, required) VALUES ($1, $2, $3, $4, $5, $6)',
		[point.photo_id, point.frame_id, point.x, point.y, point.label, point.required ? 1 : 0]
	);
	return result.lastInsertId ?? 0;
}

export async function updateFocalPoint(
	id: number,
	updates: Partial<Omit<FocalPoint, 'id' | 'photo_id' | 'created_at'>>
): Promise<void> {
	const db = await getDb();
	const sets: string[] = [];
	const values: unknown[] = [];

	if (updates.frame_id !== undefined) {
		sets.push(`frame_id = $${sets.length + 1}`);
		values.push(updates.frame_id);
	}
	if (updates.x !== undefined) {
		sets.push(`x = $${sets.length + 1}`);
		values.push(updates.x);
	}
	if (updates.y !== undefined) {
		sets.push(`y = $${sets.length + 1}`);
		values.push(updates.y);
	}
	if (updates.label !== undefined) {
		sets.push(`label = $${sets.length + 1}`);
		values.push(updates.label);
	}
	if (updates.required !== undefined) {
		sets.push(`required = $${sets.length + 1}`);
		values.push(updates.required ? 1 : 0);
	}

	if (sets.length === 0) return;

	values.push(id);
	await db.execute(`UPDATE focal_points SET ${sets.join(', ')} WHERE id = $${values.length}`, values);
}

export async function deleteFocalPoint(id: number): Promise<void> {
	const db = await getDb();
	await db.execute('DELETE FROM focal_points WHERE id = $1', [id]);
}

export async function deleteFrame(id: number): Promise<void> {
	const db = await getDb();
	await db.execute('DELETE FROM frames WHERE id = $1', [id]);
}

export async function deletePhoto(id: number): Promise<void> {
	const db = await getDb();
	await db.execute('DELETE FROM photos WHERE id = $1', [id]);
}

export async function importFile(
	filePath: string,
	metadata?: { width?: number; height?: number; title?: string; lr_asset_id?: string }
): Promise<number> {
	return addPhoto({
		file_path: filePath,
		width: metadata?.width,
		height: metadata?.height,
		title: metadata?.title || filePath.split('/').pop() || 'Untitled',
		description: undefined,
		lr_asset_id: metadata?.lr_asset_id
	});
}
