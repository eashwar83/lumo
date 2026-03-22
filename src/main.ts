import { createApp } from "vue";
import App from "./App.vue";
// import "./styles.css";

if (!import.meta.env.DEV) {
    window.addEventListener("contextmenu", (event) => {
        event.preventDefault();
    });
}

createApp(App).mount("#app");
