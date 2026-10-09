import { mount } from "svelte";
import App from "./App.svelte";
import "./app.css";
import "./workspace.css";
import "./finish.css";
mount(App, { target: document.getElementById("app")! });
