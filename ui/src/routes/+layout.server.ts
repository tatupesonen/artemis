import type { LayoutServerLoad, LayoutData } from './$types';
export const prerender=  true;

const loadStuff = async () => {
	return await fetch("http://localhost:3000/feeds").then(e => e.json());
}

export const load = (async () => {
	return {
		posts: await loadStuff(),
	};
}) satisfies LayoutServerLoad;
