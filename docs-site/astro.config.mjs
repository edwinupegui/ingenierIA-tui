// @ts-check
import { defineConfig } from 'astro/config';
import starlight from '@astrojs/starlight';

export default defineConfig({
	integrations: [
		starlight({
			title: 'ingenierIA TUI',
			defaultLocale: 'root',
			locales: { root: { label: 'Español', lang: 'es' } },
			components: {
				Header: './src/components/Header.astro',
			},
			customCss: ['./src/styles/custom.css'],
			social: [
				{ icon: 'github', label: 'GitHub', href: 'https://github.com/your-org/ingenieria-tui' },
			],
			sidebar: [
				{
					label: 'Guia',
					autogenerate: { directory: 'guia' },
				},
				{
					label: 'Funcionalidades',
					autogenerate: { directory: 'funcionalidades' },
				},
				{
					label: 'Referencia',
					autogenerate: { directory: 'reference' },
				},
				{
					label: 'Workflows',
					autogenerate: { directory: 'workflows' },
				},
			],
		}),
	],
});
