import React from "react";
import { LogIn, KeyRound, Users, ShieldCheck } from "lucide-react";
import { api, saveAdminToken, clearAdminToken } from "../api";
import { Page, Section, Field, Actions, Btn, Msg, errMsg } from "../ui";
import { route } from "../app";

export function AdminLoginPage() {
  const [username, setU] = React.useState("");
  const [password, setP] = React.useState("");
  const [bootU, setBU] = React.useState("");
  const [bootP, setBP] = React.useState("");
  const [showBoot, setShowBoot] = React.useState(false);
  const [busy, setB] = React.useState(false);
  const [m, setM] = React.useState<{ t: "ok" | "bad" | "info"; s: string } | null>(null);

  async function login() {
    setB(true);
    setM(null);
    try {
      const r = await api.adminLogin({ username, password });
      saveAdminToken(r.data.token, r.data.staff);
      route("#/admin");
    } catch (e) {
      setM({ t: "bad", s: errMsg(e) });
    } finally {
      setB(false);
    }
  }

  async function bootstrap() {
    setB(true);
    setM(null);
    try {
      const r = await api.bootstrap({ username: bootU, password: bootP });
      saveAdminToken(r.data.token, r.data.staff);
      route("#/admin");
    } catch (e) {
      setM({ t: "bad", s: errMsg(e) });
    } finally {
      setB(false);
    }
  }

  return (
    <Page title="ADMIN" sub="staff access">
      <Section title="staff login">
        <Field label="username"><input value={username} onChange={(e) => setU(e.target.value)} autoFocus /></Field>
        <Field label="password">
          <input type="password" value={password} onChange={(e) => setP(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && !busy && login()} />
        </Field>
        <Actions>
          <Btn disabled={busy} onClick={login}><LogIn /> sign in</Btn>
          <Btn onClick={() => route("#/login")}><Users /> tenant view</Btn>
        </Actions>
      </Section>

      <Section title="first-run bootstrap">
        {!showBoot ? (
          <Actions><Btn onClick={() => setShowBoot(true)}>show bootstrap</Btn></Actions>
        ) : (
          <>
            <Field label="username"><input value={bootU} onChange={(e) => setBU(e.target.value)} /></Field>
            <Field label="password"><input type="password" value={bootP} onChange={(e) => setBP(e.target.value)} /></Field>
            <Actions><Btn disabled={busy} onClick={bootstrap}><ShieldCheck /> create first admin</Btn></Actions>
          </>
        )}
        <Msg m={m} />
      </Section>
    </Page>
  );
}

export function AdminLogoutBtn() {
  return (
    <Btn
      onClick={() => {
        clearAdminToken();
        route("#/admin/login");
      }}
    >
      log out
    </Btn>
  );
}
