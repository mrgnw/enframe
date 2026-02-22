<script lang="ts">
import '../app.css';
import { Toaster } from '$lib/components/ui/sonner';

let { children } = $props();
</script>

{#if children}
    {@render children()}
{:else}
    <p>Fallback content</p>
{/if}

<Toaster />
