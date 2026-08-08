export interface Destination {
	slug: string;
	name: string;
	ratio: { w: number; h: number };
}

export const DESTINATIONS: Destination[] = [
	{ slug: 'mobile', name: 'Mobile', ratio: { w: 9, h: 16 } },
	{ slug: 'desktop', name: 'Desktop', ratio: { w: 16, h: 9 } },
	{ slug: 'instagram', name: 'Instagram', ratio: { w: 4, h: 5 } },
	{ slug: 'og', name: 'OG', ratio: { w: 1.91, h: 1 } },
	{ slug: 'square', name: 'Square', ratio: { w: 1, h: 1 } }
];

export const REQUIRED_DESTINATIONS = ['mobile', 'desktop'];

const RATIO_TOLERANCE = 0.02;

export function findDestination(slug: string | null | undefined): Destination | undefined {
	if (!slug) return undefined;
	return DESTINATIONS.find((d) => d.slug === slug);
}

export function destinationHasRatio(slug: string | null | undefined, aspect: number): boolean {
	const destination = findDestination(slug);
	if (!destination) return false;
	return Math.abs(destination.ratio.w / destination.ratio.h - aspect) <= RATIO_TOLERANCE;
}
