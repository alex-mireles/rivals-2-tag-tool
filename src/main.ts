import { createApp } from 'vue';
import App from './App.vue';
import '@fontsource-variable/inter';
import '@fontsource-variable/ubuntu-sans-mono';
import './styles/global.scss';
import { install as installBrowserFixtures } from './dev/browserMock';

// No-op inside the desktop app; in a plain browser it stands in for the Rust
// side so the UI can be worked on with `npm run dev` alone.
installBrowserFixtures();

createApp(App).mount('#app');
