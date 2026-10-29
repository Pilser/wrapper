import React from "react";
import {
  api,
  saveTenantToken,
  clearTenantToken,
  tenantToken,
  ApiError,
  statusClass,
} from "../api";
import { Page, Section, Field, Actions, Btn, Msg, PasswordInput, errMsg, RowLine } from "../ui";
import { route } from "../app";
import {
  LogIn,
  UserPlus,
  KeyRound,
  LogOut,
  Power,
  ExternalLink,
  RefreshCw,
  Monitor,
  Play,
} from "lucide-react";

function selfInfo(): any | null {
  try {
    return JSON.parse(localStorage.getItem("wrap.tenant_self") ?? "null");
  } catch {
    return null;
  }
}

export function LoginPage() {
  const [username, setU] = React.useState("");
  const [password, setP] = React.useState("");
  const [busy, setB] = React.useState(false);
  const [m, setM] = React.useState<{ t: "ok" | "bad" | "info"; s: string } | null>(null);

  async function submit() {
    setB(true);
    setM(null);
    try {
      const r = await api.login({ username, password });
      saveTenantToken(r.data.token, r.data.tenant);
      if (r.data.apps_url) localStorage.setItem("wrap.apps_url", r.data.apps_url);
      route("#/");
    } catch (e) {
      setM({ t: "bad", s: errMsg(e) });
    } finally {
      setB(false);
    }
  }

  return (
    <Page title="SIGN IN">
      <Section title="welcome back" note="your machine wakes with you">
        <Field label="username">
          <input value={username} onChange={(e) => setU(e.target.value)} autoFocus />
        </Field>
        <Field label="password">
          <PasswordInput
            value={password}
            onChange={setP}
            onEnter={() => !busy && submit()}
          />
        </Field>
        <Actions>
          <Btn disabled={busy} onClick={submit}><LogIn /> sign in</Btn>
          <Btn onClick={() => route("#/signup")}><UserPlus /> sign up instead</Btn>
          <Btn className="gold" onClick={() => route("#/admin/login")}><KeyRound /> staff</Btn>
        </Actions>
        <Msg m={m} />
      </Section>
    </Page>
  );
}

export function SignupPage() {
  const [f, setF] = React.useState({ username: "", password: "", display_name: "" });
  const [busy, setB] = React.useState(false);
  const [m, setM] = React.useState<{ t: "ok" | "bad" | "info"; s: string } | null>(null);
  const set = (k: string) => (e: any) => setF({ ...f, [k]: e.target.value });

  async function submit() {
    setB(true);
    setM(null);
    try {
      const r = await api.signup({
        username: f.username,
        password: f.password,
        display_name: f.display_name || undefined,
      });
      // Auto-login with the credentials just created — the user goes
      // straight to their dashboard (login also pre-warms the VM).
      try {
        const lr = await api.login({ username: f.username, password: f.password });
        saveTenantToken(lr.data.token, lr.data.tenant);
        if (lr.data.apps_url) localStorage.setItem("wrap.apps_url", lr.data.apps_url);
        route("#/");
        return;
      } catch {
        setM({ t: "bad", s: `account created for ${r.data.tenant?.username ?? f.username}, but auto sign-in failed — please log in` });
      }
    } catch (e) {
      setM({ t: "bad", s: errMsg(e) });
    } finally {
      setB(false);
    }
  }

  return (
    <Page title="CREATE ACCOUNT">
      <Section title="new tenant" note="username 3+ chars, password 8+ chars">
        <Field label="username"><input value={f.username} onChange={set("username")} autoFocus /></Field>
        <Field label="password">
          <PasswordInput value={f.password} onChange={(v) => setF({ ...f, password: v })} />
        </Field>
        <Field label="display name"><input value={f.display_name} onChange={set("display_name")} /></Field>
        <Actions>
          <Btn disabled={busy} onClick={submit}><UserPlus /> create account</Btn>
          <Btn onClick={() => route("#/login")}><LogIn /> login instead</Btn>
        </Actions>
        <Msg m={m} />
      </Section>
    </Page>
  );
}

export function TenantHome() {
  const self = selfInfo();
  const [me, setMe] = React.useState<any>(null);
  const [status, setStatus] = React.useState<any>(null);
  const [checkedAt, setCheckedAt] = React.useState<string>("");
  const [waking, setWaking] = React.useState(false);
  const [m, setM] = React.useState<{ t: "ok" | "bad" | "info"; s: string } | null>(null);
  const [err, setErr] = React.useState<string>("");

  const load = React.useCallback(async () => {
    try {
      const mine = (await api.me()).data;
      setMe(mine);
      setErr("");
      if (mine.apps_url) localStorage.setItem("wrap.apps_url", mine.apps_url);
    } catch (e) {
      // session gone: send back to sign in. server hiccup: show and retry.
      if (e instanceof ApiError && e.status === 401) {
        clearTenantToken();
        route("#/login");
        return;
      }
      setErr("status check failed (server hiccup), retrying");
    }
    try {
      setStatus((await api.vmStatus()).data);
      setCheckedAt(new Date().toLocaleTimeString());
    } catch {
      setErr("status check failed (server hiccup), retrying");
    }
  }, []);

  React.useEffect(() => {
    load();
    const t = setInterval(load, 5000);
    return () => clearInterval(t);
  }, [load]);

  async function wake() {
    setWaking(true);
    setM(null);
    try {
      const r = await api.vmWake();
      setM({ t: "ok", s: `machine is up at ${r.data.ip}` });
      load();
    } catch (e) {
      const d = e instanceof ApiError ? e.data : null;
      setM({ t: "bad", s: `wake failed${d?.detail ? `: ${d.detail}` : ""}` });
    } finally {
      setWaking(false);
    }
  }

  async function logout() {
    try { await api.logout(); } catch { /* session may be gone */ }
    clearTenantToken();
    route("#/login");
  }

  const appsUrl = me?.apps_url ?? localStorage.getItem("wrap.apps_url") ?? "";
  const vmSt = status?.status ?? me?.vm?.status ?? "unknown";
  const up = vmSt === "running";

  return (
    <Page title="DASHBOARD" sub={self?.username ?? me?.tenant?.username ?? ""}>
      {/* status hero: the one thing you glance at */}
      <div className="hero">
        <div className={"hero-dot " + statusClass(vmSt)} />
        <div className="hero-main">
          <span className="hero-status">{vmSt}</span>
          <span className="muted">
            {err
              ? err
              : up
              ? `machine is live at ${status?.ip ?? me?.vm?.ip ?? "..."}. checked ${checkedAt}`
              : "machine is parked. wake it below, first request also wakes it"}
          </span>
        </div>
        <div className="hero-actions">
          <Btn disabled={waking || up} onClick={wake}><Power /> wake</Btn>
          <Btn className={up ? "gold" : ""} disabled={!up} onClick={() => route("#/apps")}>
            <Play /> open apps
          </Btn>
          <Btn onClick={load}><RefreshCw /> check now</Btn>
        </div>
      </div>

      <Section title="machine" note={<><Monitor /> refreshes every 5s</>}>
        <RowLine
          items={[
            ["state", <span className={statusClass(vmSt)}>{vmSt === "running" ? "on — ready to use" : vmSt === "asleep" ? "asleep — wakes on first use" : vmSt}</span>],
            ["address", <span className="mono">{status?.ip ?? me?.vm?.ip ?? "none yet"}</span>],
            ["power", <span className="muted">sleeps automatically when idle; nothing is lost — waking brings everything back</span>],
          ]}
        />
        <Msg m={m} />
      </Section>

      <Section title="account">
        <RowLine
          items={[
            ["signed in as", self?.username ?? me?.tenant?.username ?? "none"],
            ["display name", self?.display_name ?? me?.tenant?.display_name ?? "none"],
            ["member since", self?.created_at ?? me?.tenant?.created_at ?? "none"],
            ["last login", self?.last_login ?? me?.tenant?.last_login ?? "none"],
            ["apps link", appsUrl ? <a href={appsUrl} target="_blank" rel="noreferrer">{appsUrl}</a> : "none"],
          ]}
        />
        <Actions>
          <Btn className={undefined} onClick={() => route("#/apps")} disabled={!appsUrl}><Play /> run apps here</Btn>
          <Btn onClick={logout}><LogOut /> sign out</Btn>
          <Btn className="gold" onClick={() => route("#/admin/login")}><KeyRound /> staff</Btn>
        </Actions>
      </Section>
    </Page>
  );
}
