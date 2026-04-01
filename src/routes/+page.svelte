<script lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { onMount } from 'svelte';
import { goto } from '$app/navigation';
import PhotoBrowser from '$lib/components/PhotoBrowser.svelte';

onMount(async () => {
	try {
		const photoId = await invoke<number | null>('get_screenshot_mode');
		if (photoId) {
			goto(`/edit/${photoId}`);
		}
	} catch {
		// Not in screenshot mode or not running in Tauri
	}
});
</script>

<PhotoBrowser />
