import { createApp } from "vue";
import App from "./App.vue";
// import "./styles.css";

const userAgent = window.navigator.userAgent;

if (/\bwindows\b/i.test(userAgent)) {
    document.documentElement.setAttribute("data-platform", "windows");
} else if (/\blinux\b/i.test(userAgent)) {
    document.documentElement.setAttribute("data-platform", "linux");
} else if (/mac|darwin/i.test(userAgent)) {
    document.documentElement.setAttribute("data-platform", "macos");
}

if (!import.meta.env.DEV) {
    window.addEventListener("contextmenu", (event) => {
        event.preventDefault();
    });
}

createApp(App).mount("#app");
