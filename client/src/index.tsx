import { render } from "solid-js/web";
import App from "./App";
import "./styles/global.css";

const root = document.getElementById("root");

if (!root) {
  throw new Error("Root-Element nicht gefunden");
}

render(() => <App />, root);
