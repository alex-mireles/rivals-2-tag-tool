import { createApp } from 'vue';
import { OhVueIcon, addIcons } from 'oh-vue-icons';
import {
  MdFolderopenRound,
  MdRefreshRound,
  MdFileopenRound,
  MdCheckRound,
  MdCloseRound,
  MdArrowbackRound,
  MdDownloadRound,
  MdUploadRound,
} from 'oh-vue-icons/icons/md';
import App from './App.vue';
import '@fontsource-variable/inter';
import '@fontsource-variable/ubuntu-sans-mono';
import './styles/global.scss';

// The `md` pack has 10k+ icons; import only the ones used, by name,
// e.g. `import { MdHome } from 'oh-vue-icons/icons/md';`
addIcons(
  MdFolderopenRound,
  MdRefreshRound,
  MdFileopenRound,
  MdCheckRound,
  MdCloseRound,
  MdArrowbackRound,
  MdDownloadRound,
  MdUploadRound,
);

const app = createApp(App);
app.component('v-icon', OhVueIcon);
app.mount('#app');
