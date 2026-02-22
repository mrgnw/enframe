<script lang="ts">
import type { FocalPoint, Frame, Photo } from '$lib/db';
import { exportFramesToDirectory, exportMetadata as exportMetadataFile } from '$lib/export';

interface Props {
	photo: Photo;
	frames: Frame[];
	focalPoints: FocalPoint[];
	onClose: () => void;
}

let { photo, frames, focalPoints, onClose }: Props = $props();

let exportFormat = $state<'jpeg' | 'metadata' | 'both'>('both');
let jpegQuality = $state(0.92);
let exporting = $state(false);
let exportPath = $state('');
let exportProgress = $state({ current: 0, total: 0 });

async function handleExport() {
	exporting = true;
	try {
		if (frames.length === 0) {
			alert('No frames to export');
			return;
		}

		const paths = await exportFramesToDirectory(photo, frames, jpegQuality, (current, total) => {
			exportProgress = { current, total };
		});

		if (paths.length > 0 && exportFormat !== 'jpeg') {
			// Also export metadata to the same directory
			const directory = paths[0].substring(0, paths[0].lastIndexOf('/'));
			const basename = photo.title?.replace(/\.[^/.]+$/, '').replace(/\s+/g, '-') || 'photo';
			const metadataPath = `${directory}/${basename}.metadata.json`;
			await exportMetadataFile(photo, frames, focalPoints, metadataPath);
		}

		alert(`Exported ${paths.length} ${paths.length === 1 ? 'frame' : 'frames'}!`);
		onClose();
	} catch (error) {
		console.error('Export failed:', error);
		alert(`Export failed: ${error}`);
	} finally {
		exporting = false;
	}
}
</script>

<div class="dialog-overlay" onclick={onClose}>
	<div class="dialog" onclick={(e) => e.stopPropagation()}>
		<div class="dialog-header">
			<h2>Export</h2>
			<button class="close-btn" onclick={onClose}>×</button>
		</div>

		<div class="dialog-content">
			<div class="field">
				<label>Export Format</label>
				<div class="radio-group">
					<label>
						<input type="radio" bind:group={exportFormat} value="both" />
						Both (Crops + Metadata)
					</label>
					<label>
						<input type="radio" bind:group={exportFormat} value="jpeg" />
						JPEG Crops Only
					</label>
					<label>
						<input type="radio" bind:group={exportFormat} value="metadata" />
						Metadata JSON Only
					</label>
				</div>
			</div>

			{#if exportFormat !== 'metadata'}
				<div class="field">
					<label for="quality">JPEG Quality: {Math.round(jpegQuality * 100)}%</label>
					<input
						id="quality"
						type="range"
						min="0.5"
						max="1"
						step="0.01"
						bind:value={jpegQuality}
					/>
				</div>
			{/if}

			<div class="info">
				<p><strong>{frames.length}</strong> frame{frames.length === 1 ? '' : 's'} to export</p>
				<p><strong>{focalPoints.length}</strong> focal point{focalPoints.length === 1 ? '' : 's'}</p>
				{#if exporting && exportProgress.total > 0}
					<p>Exporting {exportProgress.current} of {exportProgress.total}...</p>
				{/if}
			</div>
		</div>

		<div class="dialog-footer">
			<button class="secondary" onclick={onClose} disabled={exporting}>Cancel</button>
			<button class="primary" onclick={handleExport} disabled={exporting}>
				{exporting ? 'Exporting...' : 'Export'}
			</button>
		</div>
	</div>
</div>

<style>
	.dialog-overlay {
		position: fixed;
		top: 0;
		left: 0;
		right: 0;
		bottom: 0;
		background: rgba(0, 0, 0, 0.5);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 1000;
	}

	.dialog {
		background: white;
		border-radius: 0.5rem;
		box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.1);
		min-width: 400px;
		max-width: 500px;
	}

	.dialog-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		padding: 1.5rem;
		border-bottom: 1px solid #e5e7eb;
	}

	.dialog-header h2 {
		margin: 0;
		font-size: 1.25rem;
	}

	.close-btn {
		background: none;
		border: none;
		font-size: 1.5rem;
		cursor: pointer;
		color: #6b7280;
		padding: 0;
		width: 2rem;
		height: 2rem;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.close-btn:hover {
		color: #374151;
	}

	.dialog-content {
		padding: 1.5rem;
	}

	.field {
		margin-bottom: 1.5rem;
	}

	.field label {
		display: block;
		margin-bottom: 0.5rem;
		font-weight: 500;
		color: #374151;
	}

	.radio-group {
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.radio-group label {
		display: flex;
		align-items: center;
		gap: 0.5rem;
		font-weight: normal;
		cursor: pointer;
	}

	input[type="range"] {
		width: 100%;
	}

	.info {
		background: #f3f4f6;
		padding: 1rem;
		border-radius: 0.375rem;
		font-size: 0.875rem;
	}

	.info p {
		margin: 0.25rem 0;
	}

	.dialog-footer {
		display: flex;
		justify-content: flex-end;
		gap: 0.75rem;
		padding: 1.5rem;
		border-top: 1px solid #e5e7eb;
	}

	button {
		padding: 0.5rem 1rem;
		border-radius: 0.375rem;
		cursor: pointer;
		font-size: 0.875rem;
		border: none;
	}

	button.primary {
		background: #2563eb;
		color: white;
	}

	button.primary:hover:not(:disabled) {
		background: #1d4ed8;
	}

	button.secondary {
		background: #f3f4f6;
		color: #374151;
	}

	button.secondary:hover:not(:disabled) {
		background: #e5e7eb;
	}

	button:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
</style>
