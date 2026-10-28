import { ChangesBar, Sidebar, StatusBanners, Toast } from "./components/Shell";
import { AppRules } from "./pages/AppRules";
import { Dashboard } from "./pages/Dashboard";
import { Macros } from "./pages/Macros";
import { Profiles } from "./pages/Profiles";
import { Settings } from "./pages/Settings";
import { Studio } from "./pages/Studio";
import { StoreProvider, useStore } from "./store";

function Main() {
  const { state } = useStore();
  if (!state.draft || !state.status) {
    return <div className="grid h-screen place-items-center text-sm text-zinc-500">{state.toast?.text ?? "Loading…"}</div>;
  }
  const Page = { dashboard: Dashboard, studio: Studio, macros: Macros, profiles: Profiles, apps: AppRules, settings: Settings }[state.page];
  return (
    <div className="flex h-screen">
      <Sidebar />
      <div className="flex min-w-0 flex-1 flex-col">
        <main className="flex-1 overflow-y-auto px-6 py-6">
          <div className="mx-auto grid max-w-6xl gap-4">
            <StatusBanners />
            <Page />
          </div>
        </main>
        <ChangesBar />
      </div>
      <Toast />
    </div>
  );
}

export default function App() {
  return (
    <StoreProvider>
      <Main />
    </StoreProvider>
  );
}
