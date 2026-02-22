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

export async function listLightroomPreviews(): Promise<LightroomPhoto[]> {
	try {
		const libraryPath = await getLightroomLibraryPath();
		
		// Find the catalog GUID directory
		const catalogPath = `${libraryPath}/00000000000000000000000000000000`;
		const previewsDbPath = `sqlite:${catalogPath}/previews.db`;
		
		const db = await Database.load(previewsDbPath);
		
		// Query previews - get the largest preview for each asset
		const rows = await db.select<{
			localAssetId: string;
			longSide: number;
			path: string;
		}[]>(`
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
		
		return rows.map(row => ({
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
