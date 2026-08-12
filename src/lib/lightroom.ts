import { invoke } from './bridge';

export interface LrAlbum {
	album_id: string;
	name: string;
	parent_id: string | null;
	subtype: string | null;
	asset_count: number;
}

export interface LrAsset {
	asset_id: string;
	filename: string | null;
	width: number | null;
	height: number | null;
	orientation: number | null;
	capture_date: string | null;
	rating: number | null;
	flag: number | null;
	has_local_original: boolean;
	has_local_proxy: boolean;
}

export interface LrAssetPaths {
	asset_id: string;
	original_path: string | null;
	proxy_path: string | null;
	xmp_path: string | null;
	preview_path: string | null;
}

export function getLightroomLibraryPath(): Promise<string> {
	return invoke<string>('get_lightroom_library_path');
}

export function listLrAlbums(): Promise<LrAlbum[]> {
	return invoke<LrAlbum[]>('list_lr_albums');
}

export function listLrAlbumAssets(albumId: string, limit?: number, offset?: number): Promise<LrAsset[]> {
	return invoke<LrAsset[]>('list_lr_album_assets', {
		albumId,
		limit: limit ?? null,
		offset: offset ?? null
	});
}

export function searchLrAssets(query: string, limit?: number): Promise<LrAsset[]> {
	return invoke<LrAsset[]>('search_lr_assets', {
		query,
		limit: limit ?? null
	});
}

export function listLrRecent(limit?: number): Promise<LrAsset[]> {
	return invoke<LrAsset[]>('list_lr_recent', {
		limit: limit ?? null
	});
}

export function getLrAssetPaths(assetIds: string[]): Promise<LrAssetPaths[]> {
	return invoke<LrAssetPaths[]>('get_lr_asset_paths', { assetIds });
}

export function bestLocalPath(paths: LrAssetPaths): string | null {
	return paths.original_path ?? paths.proxy_path ?? paths.preview_path ?? null;
}
