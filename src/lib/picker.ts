import { mount, unmount } from 'svelte';
import FilePicker from './library/FilePicker.svelte';

export function browserPick(options: {
	title?: string;
	directory?: boolean;
	multiple?: boolean;
	extensions?: string[];
}): Promise<string[] | null> {
	return new Promise((resolve) => {
		const target = document.createElement('div');
		document.body.appendChild(target);
		const picker = mount(FilePicker, {
			target,
			props: {
				...options,
				onclose: (selected: string[] | null) => {
					unmount(picker);
					target.remove();
					resolve(selected);
				}
			}
		});
	});
}
