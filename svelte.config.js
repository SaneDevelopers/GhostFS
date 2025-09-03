import adapter from '@sveltejs/adapter-static';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	kit: {
		adapter: adapter({
			pages: 'gui-dist',
			assets: 'gui-dist',
			fallback: 'index.html',
			precompress: false,
			strict: true
		})
	}
};

export default config;
