import { createApp } from "vue";
import OpggWindow from "./components/OpggWindow.vue";

import i18n from "./i18n";

const app = createApp(OpggWindow);
app.use(i18n);
app.mount("#app");
