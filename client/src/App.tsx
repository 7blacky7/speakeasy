import { Route, Router } from "@solidjs/router";
import { lazy } from "solid-js";
import MainLayout from "./layouts/MainLayout";

const ServerBrowser = lazy(() => import("./pages/ServerBrowser"));
const ServerView = lazy(() => import("./pages/ServerView"));
const Settings = lazy(() => import("./pages/Settings"));
const AudioSettings = lazy(() => import("./pages/AudioSettings"));
const PluginSettings = lazy(() => import("./pages/PluginSettings"));

export default function App() {
  return (
    <Router root={MainLayout}>
      <Route path="/" component={ServerBrowser} />
      <Route path="/server/:id" component={ServerView} />
      <Route path="/settings" component={Settings} />
      <Route path="/settings/audio" component={AudioSettings} />
      <Route path="/settings/plugins" component={PluginSettings} />
    </Router>
  );
}
