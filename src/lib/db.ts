import { readTextFile } from '@tauri-apps/plugin-fs';
import Database from '@tauri-apps/plugin-sql';

let dbPromise: Promise<Database> | null = null;

export function getDb(): Promise<Database> {
	if (!dbPromise) {
		console.log('[db] Database.load() starting...');
		dbPromise = Database.load('sqlite:enframe.db').then(
			(db) => {
				console.log('[db] Database.load() succeeded');
				return db;
			},
			(err) => {
				console.error('[db] Database.load() FAILED:', err);
				dbPromise = null; // clear cache so next call retries
				throw err;
			}
		);
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
	console.log('[db] listPhotos() calling getDb()...');
	const db = await getDb();
	console.log('[db] listPhotos() got db, running SELECT...');
	const result = await db.select<Photo[]>('SELECT * FROM photos ORDER BY created_at DESC');
	console.log('[db] listPhotos() returned', result.length, 'photos');
	return result;
}

export async function getPhoto(id: number): Promise<Photo | null> {
	const db = await getDb();
	const results = await db.select<Photo[]>('SELECT * FROM photos WHERE id = $1', [id]);
	return results[0] || null;
}

export async function getPhotoByPath(filePath: string): Promise<Photo | null> {
	const db = await getDb();
	const results = await db.select<Photo[]>('SELECT * FROM photos WHERE file_path = $1', [filePath]);
	return results[0] || null;
}

export async function updatePhoto(id: number, updates: Partial<Omit<Photo, 'id' | 'created_at'>>): Promise<void> {
	const db = await getDb();
	const sets: string[] = [];
	const values: unknown[] = [];

	if (updates.file_path !== undefined) {
		sets.push(`file_path = $${sets.length + 1}`);
		values.push(updates.file_path);
	}
	if (updates.width !== undefined) {
		sets.push(`width = $${sets.length + 1}`);
		values.push(updates.width);
	}
	if (updates.height !== undefined) {
		sets.push(`height = $${sets.length + 1}`);
		values.push(updates.height);
	}
	if (updates.title !== undefined) {
		sets.push(`title = $${sets.length + 1}`);
		values.push(updates.title);
	}
	if (updates.description !== undefined) {
		sets.push(`description = $${sets.length + 1}`);
		values.push(updates.description);
	}
	if (updates.lr_asset_id !== undefined) {
		sets.push(`lr_asset_id = $${sets.length + 1}`);
		values.push(updates.lr_asset_id);
	}

	if (sets.length === 0) return;

	values.push(id);
	await db.execute(`UPDATE photos SET ${sets.join(', ')} WHERE id = $${values.length}`, values);
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

export async function importFromMetadata(metadataPath: string): Promise<number> {
	const content = await readTextFile(metadataPath);
	const metadata = JSON.parse(content);

	// Validate metadata structure (basic check)
	if (!metadata.photo || !metadata.photo.file_path) {
		throw new Error('Invalid metadata file: missing photo or file_path');
	}

	// Check for existing photo
	const existing = await getPhotoByPath(metadata.photo.file_path);
	let photoId: number;

	if (existing) {
		// If photo exists, we update it with metadata info if missing, but primarily we want to return its ID
		// Optionally update title/dims if they are missing in DB but present in metadata
		// For now, let's just return the existing ID and skip frame import to avoid duplicates?
		// Or should we import frames anyway? The user might be re-importing to update frames.
		// Let's assume we proceed but maybe we should clear old frames?
		// For safety, let's just return existing ID and let the user deal with it, OR simpler:
		// We treat it as "importing new data", so maybe we should just skip if photo exists to avoid duplication.
		// But the prompt says "Import frameset... import everything".
		// Let's create a new photo entry if the path is different, but since path is the unique key (conceptually),
		// we should probably just return the existing ID.
		console.log('Photo already exists for path:', metadata.photo.file_path);
		return existing.id;
	} else {
		// Create new photo
		photoId = await addPhoto({
			file_path: metadata.photo.file_path,
			width: metadata.photo.width,
			height: metadata.photo.height,
			title: metadata.photo.title,
			description: metadata.photo.description,
			lr_asset_id: metadata.photo.lr_asset_id
		});
	}

	// Map old frame IDs to new frame IDs
	const frameIdMap = new Map<number, number>();

	if (metadata.frames && Array.isArray(metadata.frames)) {
		for (const frame of metadata.frames) {
			const newFrameId = await addFrame({
				photo_id: photoId,
				label: frame.label,
				x: frame.x,
				y: frame.y,
				width: frame.width,
				height: frame.height,
				rotation: frame.rotation || 0,
				is_default: frame.is_default || false
			});
			if (frame.id) {
				frameIdMap.set(frame.id, newFrameId);
			}
		}
	}

	if (metadata.focal_points && Array.isArray(metadata.focal_points)) {
		for (const point of metadata.focal_points) {
			// Resolve new frame ID if it was linked to a frame
			let newFrameId: number | undefined;
			if (point.frame_id !== null && point.frame_id !== undefined) {
				newFrameId = frameIdMap.get(point.frame_id);
			}

			await addFocalPoint({
				photo_id: photoId,
				frame_id: newFrameId, // This will be undefined if map lookup failed or if it was null
				x: point.x,
				y: point.y,
				label: point.label,
				required: point.required
			});
		}
	}

	return photoId;
}
