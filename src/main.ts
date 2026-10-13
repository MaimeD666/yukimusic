import { mount } from "svelte";
import App from "./App.svelte";
import "./app.css";
import "./player.css";
import "./motion.css";
import { applyTheme, readTheme } from "./lib/catalog";
applyTheme(readTheme());
mount(App, { target: document.getElementById("app")! });
