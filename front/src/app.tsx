import React from "react";
import {
  adminToken,
  tenantToken,
  KEYS,
} from "./api";
import {
  Leaf,
  House,
  LogIn,
  UserPlus,
  Shield,
  KeyRound,
  LayoutDashboard,
  Users,
  Tags,
  Boxes,
  Server,
  Package,
  FileText,
  Share2,
  Activity,
  Sun,
  Moon,
  Play,
} from "lucide-react";
import { LoginPage, SignupPage, TenantHome } from "./pages/tenant";
import { AppsFrame } from "./pages/apps-frame";
import { AdminLoginPage } from "./pages/admin-login";
import { AdminTenants } from "./pages/tenants";
import { AdminTemplates } from "./pages/templates";
import { AdminCategories } from "./pages/categories";
import { AdminBinaries } from "./pages/binaries";
import { AdminFleet } from "./pages/fleet";
import { AdminPrompt } from "./pages/prompt";
import { AdminHub } from "./pages/hub";
import { AdminDashboard } from "./pages/dashboard";
import { AdminLogs } from "./pages/logs";

// tiny hash router: paths like #/admin/tenants
export function route(to: string) {
  location.hash = to;
}

function useHash(): string {
  const [h, setH] = React.useState(location.hash || "#/");
  React.useEffect(() => {
    const on = () => setH(location.hash || "#/");
    window.addEventListener("hashchange", on);
    return () => window.removeEventListener("hashchange", on);
  }, []);
  return h;
}

function ThemeToggle() {
  const apply = (t: string) => document.documentElement.setAttribute("data-theme", t);
  const [theme, setTheme] = React.useState(localStorage.getItem(KEYS.theme) ?? "dark");
  React.useEffect(() => { apply(theme); localStorage.setItem(KEYS.theme, theme); }, [theme]);
  const isDark = theme === "dark";
  return (
    <button
      className="btn bare"
      onClick={() => setTheme(isDark ? "light" : "dark")}
      title={isDark ? "switch to cream light" : "switch to forest dark"}
    >
      {isDark ? <Sun /> : <Moon />}
      {isDark ? "light" : "dark"}
    </button>
  );
}

function Nav(props: { hash: string }) {
  const { hash } = props;
  const hasTenant = !!tenantToken();
  const hasAdmin = !!adminToken();
  const inAdmin = hash.startsWith("#/admin");

  if (inAdmin) {
    return (
      <>
        <div className="navhead admin">Admin console</div>
        {hasAdmin ? (
          <>
            <a href="#/admin" className={hash === "#/admin" ? "on" : ""}><LayoutDashboard /> overview</a>
            <a href="#/admin/tenants" className={hash === "#/admin/tenants" ? "on" : ""}><Users /> tenants</a>
            <a href="#/admin/categories" className={hash === "#/admin/categories" ? "on" : ""}><Tags /> categories</a>
            <a href="#/admin/templates" className={hash === "#/admin/templates" ? "on" : ""}><Boxes /> templates</a>
            <a href="#/admin/fleet" className={hash === "#/admin/fleet" ? "on" : ""}><Server /> machines</a>
            <a href="#/admin/binaries" className={hash === "#/admin/binaries" ? "on" : ""}><Package /> programs</a>
            <a href="#/admin/prompt" className={hash === "#/admin/prompt" ? "on" : ""}><FileText /> instructions</a>
            <a href="#/admin/hub" className={hash === "#/admin/hub" ? "on" : ""}><Share2 /> sharing</a>
            <a href="#/admin/logs" className={hash === "#/admin/logs" ? "on" : ""}><Activity /> events</a>
          </>
        ) : (
          <a href="#/admin/login" className={hash === "#/admin/login" ? "on" : ""}><KeyRound /> staff sign in</a>
        )}
        <div className="navhead">Your area</div>
        {hasTenant ? (
          <a href="#/" className={hash === "#/" ? "on" : ""}><House /> home</a>
        ) : (
          <>
            <a href="#/login" className={hash === "#/login" ? "on" : ""}><LogIn /> sign in</a>
            <a href="#/signup" className={hash === "#/signup" ? "on" : ""}><UserPlus /> create account</a>
          </>
        )}
      </>
    );
  }

  // tenant view is the default
  return (
    <>
      <div className="navhead">Your area</div>
      {hasTenant ? (
        <>
          <a href="#/" className={hash === "#/" || hash === "" ? "on" : ""}><House /> home</a>
          <a href="#/apps" className={hash === "#/apps" ? "on" : ""}><Play /> apps</a>
        </>
      ) : (
        <>
          <a href="#/login" className={hash === "#/login" ? "on" : ""}><LogIn /> sign in</a>
          <a href="#/signup" className={hash === "#/signup" ? "on" : ""}><UserPlus /> create account</a>
        </>
      )}
      <div className="navhead admin">Admin console</div>
      {hasAdmin ? (
        <a href="#/admin" className={hash === "#/admin" ? "on" : ""}><Shield /> control panel</a>
      ) : (
        <a href="#/admin/login" className={hash === "#/admin/login" ? "on" : ""}><KeyRound /> staff sign in</a>
      )}
    </>
  );
}

const PAGES: Record<string, () => React.ReactElement> = {
  "": () => (tenantToken() ? <TenantHome /> : <LoginPage />),
  "#/": () => (tenantToken() ? <TenantHome /> : <LoginPage />),
  "#/login": () => <LoginPage />,
  "#/signup": () => <SignupPage />,
  "#/apps": () => <AppsFrame />,
  "#/admin/login": () => <AdminLoginPage />,
  "#/admin": () => <AdminDashboard />,
  "#/admin/tenants": () => <AdminTenants />,
  "#/admin/categories": () => <AdminCategories />,
  "#/admin/templates": () => <AdminTemplates />,
  "#/admin/fleet": () => <AdminFleet />,
  "#/admin/binaries": () => <AdminBinaries />,
  "#/admin/prompt": () => <AdminPrompt />,
  "#/admin/hub": () => <AdminHub />,
  "#/admin/logs": () => <AdminLogs />,
};

export function App() {
  const hash = useHash();
  const inAdmin = hash.startsWith("#/admin");
  const page = PAGES[hash] ?? PAGES[""];
  // gate: admin area beyond the login page needs a staff token
  const needAdminLogin =
    inAdmin && hash !== "#/admin/login" && !adminToken();

  // fullscreen app runner: no shell, just the frame and its top bar
  if (hash === "#/apps") {
    if (!tenantToken()) return <LoginPage />;
    return <AppsFrame />;
  }

  return (
    <div className="shell">
      <div className="topbar">
        <div className="brand">
          <Leaf />
          JERBOAI
        </div>
        {adminToken() ? (
          <div className="pill">
            <Shield />
            staff
          </div>
        ) : null}
        <div className="spacer" />
        <ThemeToggle />
      </div>
      <div className="body">
        <nav className="sidenav">
          <Nav hash={hash} />
        </nav>
        <main className="main">{needAdminLogin ? <AdminLoginPage /> : page()}</main>
      </div>
    </div>
  );
}
