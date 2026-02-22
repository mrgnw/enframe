import { invoke } from '@tauri-apps/api/core';
import Database from '@tauri-apps/plugin-sql';

export interface LightroomPhoto {
	assetId: string;
	filePath?: string;
	previewPath?: string;
	width?: number;
	height?: number;
	title?: string;
	rating?: number;
}

export async function getLightroomLibraryPath(): Promise<string> {
	return invoke<string>('get_lightroom_library_path');
}

async function findCatalogGuid(libraryPath: string): Promise<string | null> {
	try {
		const { readDir } = await import('@tauri-apps/plugin-fs');
		const entries = await readDir(libraryPath);

		// Look for a directory with a GUID-like name (hex-only, ~32 chars)
		for (const entry of entries) {
			if (entry.isDirectory && entry.name.match(/^[a-f0-9]{32}$/)) {
				return entry.name;
			}
		}
		return null;
	} catch (error) {
		console.error('Failed to find catalog GUID:', error);
		return null;
	}
}

export async function listLightroomPreviews(): Promise<LightroomPhoto[]> {
	try {
		const libraryPath = await getLightroomLibraryPath();

		// Find the catalog GUID directory dynamically
		const catalogGuid = await findCatalogGuid(libraryPath);
		if (!catalogGuid) {
			throw new Error('Could not find Lightroom catalog directory');
		}

		const catalogPath = `${libraryPath}/${catalogGuid}`;
		const previewsDbPath = `sqlite:${catalogPath}/previews.db`;

		const db = await Database.load(previewsDbPath);

		// Query previews - get the largest preview for each asset
		const rows = await db.select<
			{
				localAssetId: string;
				longSide: number;
				path: string;
			}[]
		>(`
			SELECT localAssetId, longSide, path
			FROM previews
			WHERE longSide IN (
				SELECT MAX(longSide)
				FROM previews
				GROUP BY localAssetId
			)
			ORDER BY localAssetId
			LIMIT 100
		`);

		await db.close();

		return rows.map((row) => ({
			assetId: row.localAssetId,
			previewPath: row.path,
			width: row.longSide,
			height: row.longSide
		}));
	} catch (error) {
		console.error('Failed to read Lightroom previews:', error);
		return [];
	}
}
