import { createApp } from "vue";
import App from "./App.vue";
import router from "./router";
// antdv-next 组件样式: reset + CSS-in-JS 运行时注入
import "antdv-next/dist/reset.css";
import "./styles/index.scss";

createApp(App).use(router).mount("#app");
