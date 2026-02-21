import { Route, Router } from "@solidjs/router";
import { lazy } from "solid-js";
import { getCurrentWindow } from "@tauri-apps/api/window";
import MainLayout from "./layouts/MainLayout";
import SubWindowLayout from "./layouts/SubWindowLayout";

const ServerBrowser = lazy(() => import("./pages/ServerBrowser"));
const ServerView = lazy(() => import("./pages/ServerView"));
const Settings = lazy(() => import("./pages/Settings"));
const AudioSettings = lazy(() => import("./pages/AudioSettings"));
const PluginSettings = lazy(() => import("./pages/PluginSettings"));
const AccountSettings = lazy(() => import("./pages/AccountSettings"));
const AdminPanel = lazy(() => import("./pages/AdminPanel"));

const isMainWindow = getCurrentWindow().label === "main";

export default function App() {
  return (
    <Router root={isMainWindow ? MainLayout : SubWindowLayout}>
      <Route path="/" component={ServerBrowser} />
      <Route path="/server/:id" component={ServerView} />
      <Route path="/settings" component={Settings} />
      <Route path="/settings/audio" component={AudioSettings} />
      <Route path="/settings/plugins" component={PluginSettings} />
      <Route path="/settings/account" component={AccountSettings} />
      <Route path="/admin" component={AdminPanel} />
    </Router>
  );
}
