import { createApp } from "vue";
import App from "./App.vue";
// import "./styles.css";

const userAgent = window.navigator.userAgent;

if (/\bwindows\b/i.test(userAgent)) {
    document.documentElement.setAttribute("data-platform", "windows");
} else if (/\blinux\b/i.test(userAgent)) {
    document.documentElement.setAttribute("data-platform", "linux");
}

if (!import.meta.env.DEV) {
    window.addEventListener("contextmenu", (event) => {
        event.preventDefault();
    });
}

createApp(App).mount("#app");
