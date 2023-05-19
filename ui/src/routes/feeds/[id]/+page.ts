import type { PageServerLoad } from "./$types";

const loadDataForPageId = async (pageId: string) => {
	return fetch("http://localhost:3000/feeds/" + pageId).then(data => data.json());
}

export const load = (({ params }) => {
	return {
		items: loadDataForPageId(params.id)
	}
}) satisfies PageServerLoad;