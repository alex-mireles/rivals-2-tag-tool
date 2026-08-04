import { createApp } from 'vue';
import { OhVueIcon, addIcons } from 'oh-vue-icons';
import {
  MdFolderopenRound,
  MdFolderRound,
  MdRefreshRound,
  MdFileopenRound,
  MdCheckRound,
  MdCloseRound,
  MdArrowbackRound,
  MdDownloadRound,
  MdUploadRound,
  MdClouduploadRound,
  MdRestartaltRound,
  MdDoneallRound,
  MdSwaphorizRound,
  MdStopcircleRound,
  MdArchiveRound,
  MdLogoutRound,
  MdDeleteRound,
  MdPersonsearchRound,
  MdEmojieventsRound,
  MdSearchRound,
  MdExpandmoreRound,
  MdExpandlessRound,
} from 'oh-vue-icons/icons/md';
import App from './App.vue';
import '@fontsource-variable/inter';
import '@fontsource-variable/ubuntu-sans-mono';
import './styles/global.scss';

// The `md` pack has 10k+ icons; import only the ones used, by name,
// e.g. `import { MdHome } from 'oh-vue-icons/icons/md';`
addIcons(
  MdFolderopenRound,
  MdFolderRound,
  MdRefreshRound,
  MdFileopenRound,
  MdCheckRound,
  MdCloseRound,
  MdArrowbackRound,
  MdDownloadRound,
  MdUploadRound,
  MdClouduploadRound,
  MdRestartaltRound,
  MdDoneallRound,
  MdSwaphorizRound,
  MdStopcircleRound,
  MdArchiveRound,
  MdLogoutRound,
  MdDeleteRound,
  MdPersonsearchRound,
  MdEmojieventsRound,
  MdSearchRound,
  MdExpandmoreRound,
  MdExpandlessRound,
);

const app = createApp(App);
app.component('v-icon', OhVueIcon);
app.mount('#app');
