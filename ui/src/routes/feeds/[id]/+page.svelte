<script lang="ts">
	import {
		Table,
		TableBody,
		TableBodyCell,
		TableBodyRow,
		TableHead,
		TableHeadCell
	} from 'flowbite-svelte';
	import type { PageServerData } from './$types';

	export let data: PageServerData;
</script>

<Table>
	<TableHead>
		<TableHeadCell>Title</TableHeadCell>
		<TableHeadCell>Release date</TableHeadCell>
	</TableHead>
	{#await data.items}
		<div>Loading...</div>
	{:then v}
		{#each v as item}
			<TableBodyRow>
				<TableBodyCell>
					{#if item.link !== null}
						<a
							href={item.link}
							target="_blank"
							class="font-medium text-blue-600 hover:underline dark:text-blue-500"
						>
							{item.title}
						</a>
					{:else}
						{item.title}
					{/if}
				</TableBodyCell>
				<TableBodyCell>{item.pub_date}</TableBodyCell>
			</TableBodyRow>
		{/each}
	{:catch err}
		{err.message}
	{/await}
	<TableBody class="divide-y" />
</Table>