import { createApp } from "vue";
import { createPinia } from "pinia";
import ElementPlus from "element-plus";
import "element-plus/dist/index.css";
import { i18n } from "./i18n";
import App from "./App.vue";
import "./assets/styles/tailwind.css";
import "./assets/styles/fonts.css";

// Set platform class before mount for CSS layout
const platform = navigator.platform.toUpperCase();
if (platform.includes("MAC")) document.documentElement.classList.add("platform-macos");
else if (platform.includes("WIN")) document.documentElement.classList.add("platform-windows");
else document.documentElement.classList.add("platform-linux");

const app = createApp(App);
app.use(createPinia());
app.use(ElementPlus);
app.use(i18n);
app.mount("#app");
