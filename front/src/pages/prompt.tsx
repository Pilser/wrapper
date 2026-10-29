import React from "react";
import { api } from "../api";
import { FolderOpen, Save, Trash2, CloudUpload, Eye } from "lucide-react";
import { Page, Section, Field, Actions, Btn, Msg, errMsg } from "../ui";

const SCOPES = ["global", "category", "tenant"] as const;

export function AdminPrompt() {
  const [tenants, setTenants] = React.useState<any[]>([]);
  const [cats, setCats] = React.useState<any[]>([]);
  const [scope, setScope] = React.useState<string>("global");
  const [ref, setRef] = React.useState("");
  const [content, setContent] = React.useState("");
  const [source, setSource] = React.useState("");
  const [effT, setEffT] = React.useState("");
  const [eff, setEff] = React.useState<any>(null);
  // launchers
  const [scripts, setScripts] = React.useState<any>(null);
  const [lName, setLName] = React.useState<string>("zw");
  const [lContent, setLContent] = React.useState("");
  const [m, setM] = React.useState<{ t: "ok" | "bad" | "info"; s: string } | null>(null);
  const [busy, setB] = React.useState(false);

  const loadRefs = React.useCallback(async () => {
    try {
      setTenants((await api.tenantsList()).data.tenants ?? []);
      setCats((await api.categoriesList()).data.categories ?? []);
    } catch (e) {
      setM({ t: "bad", s: errMsg(e) });
    }
  }, []);
  React.useEffect(() => { loadRefs(); }, [loadRefs]);

  async function pullPrompt() {
    setM(null);
    try {
      const r = await api.promptGet(scope, ref);
      setContent(r.data.content ?? "");
      setSource(r.data.source ?? "");
    } catch (e) {
      setM({ t: "bad", s: errMsg(e) });
    }
  }

  async function savePrompt() {
    setB(true); setM(null);
    try {
      await api.promptSet({ scope, ref_id: ref || undefined, content });
      setM({ t: "ok", s: "prompt saved. use sync to send it to tenant disks" });
    } catch (e) {
      setM({ t: "bad", s: errMsg(e) });
    } finally { setB(false); }
  }

  async function delPrompt() {
    setB(true); setM(null);
    try {
      await api.promptDelete(scope, ref);
      setContent(""); setSource("");
      setM({ t: "ok", s: "override deleted (falls back to next precedence)" });
    } catch (e) {
      setM({ t: "bad", s: errMsg(e) });
    } finally { setB(false); }
  }

  async function effective() {
    setM(null);
    try {
      const r = await api.promptEffective(effT);
      setEff(r.data);
    } catch (e) {
      setM({ t: "bad", s: errMsg(e) });
    }
  }

  async function pullLaunchers() {
    setM(null);
    try {
      setScripts((await api.launchersGet(scope, ref)).data.scripts ?? {});
    } catch (e) {
      setM({ t: "bad", s: errMsg(e) });
    }
  }

  async function saveLauncher() {
    setB(true); setM(null);
    try {
      await api.launcherSet({ scope, ref_id: ref || undefined, name: lName, content: lContent });
      setM({ t: "ok", s: `launcher ${lName} saved` });
      pullLaunchers();
    } catch (e) {
      setM({ t: "bad", s: errMsg(e) });
    } finally { setB(false); }
  }

  async function delLauncher() {
    setB(true); setM(null);
    try {
      await api.launcherDelete(scope, ref, lName);
      setLContent("");
      setM({ t: "ok", s: `launcher ${lName} deleted` });
      pullLaunchers();
    } catch (e) {
      setM({ t: "bad", s: errMsg(e) });
    } finally { setB(false); }
  }

  async function bulkSync(sel: string) {
    setB(true); setM(null);
    try {
      const body = sel === "all" ? { all: true } : { category_id: sel };
      const r = await api.workspaceSyncBulk(body);
      const res = r.data.results ?? [];
      const bad = res.filter((x: any) => x.error);
      setM({
        t: bad.length ? "bad" : "ok",
        s: `synced ${res.filter((x: any) => x.written).length}/${res.length} tenants${bad.length ? `, ${bad.length} errors` : ""}`,
      });
    } catch (e) {
      setM({ t: "bad", s: errMsg(e) });
    } finally { setB(false); }
  }

  function refOptions() {
    if (scope === "tenant") return tenants.map((t) => <option key={t.id} value={t.id}>{t.username}</option>);
    if (scope === "category") return cats.map((c) => <option key={c.id} value={c.id}>{c.name}</option>);
    return [<option key="none" value="">(global needs no ref)</option>];
  }

  return (
    <Page title="SYSTEM PROMPT" sub="agent instructions and launcher scripts">
      <Section title="scope">
        <Field label="scope">
          <select value={scope} onChange={(e) => { setScope(e.target.value); setRef(""); setScripts(null); }}>
            {SCOPES.map((s) => <option key={s} value={s}>{s}</option>)}
          </select>
        </Field>
        <Field label="ref"><select value={ref} onChange={(e) => setRef(e.target.value)}>{refOptions()}</select></Field>
        <Field label="content"><textarea value={content} onChange={(e) => setContent(e.target.value)} placeholder="agent system prompt…" /></Field>
        <p className="muted">source: {source || "none"}</p>
        <Actions>
          <Btn disabled={busy} onClick={pullPrompt}><FolderOpen /> load</Btn>
          <Btn disabled={busy} onClick={savePrompt}><Save /> save</Btn>
          <Btn disabled={busy} className="bad" onClick={delPrompt}><Trash2 /> delete override</Btn>
        </Actions>
      </Section>

      <Section title="distribute">
        <Actions>
          <Btn disabled={busy} onClick={() => bulkSync("all")}><CloudUpload /> sync all tenants</Btn>
          {cats.map((c) => (
            <Btn key={c.id} disabled={busy} onClick={() => bulkSync(c.id)}><CloudUpload /> sync {c.name}</Btn>
          ))}
        </Actions>
      </Section>

      <Section title="effective probe">
        <Field label="tenant">
          <select value={effT} onChange={(e) => setEffT(e.target.value)}>
            <option value="">(select)</option>
            {tenants.map((t) => <option key={t.id} value={t.id}>{t.username}</option>)}
          </select>
        </Field>
        <Actions><Btn disabled={busy || !effT} onClick={effective}><Eye /> show</Btn></Actions>
        {eff ? (
          <div>
            <div className="rowline"><b>source</b><span>{eff.source ?? "none"}</span></div>
            <div className="rowline"><b>content</b><pre className="muted">{eff.content ?? "none"}</pre></div>
          </div>
        ) : null}
      </Section>

      <Section title="launchers">
        <Actions>
          <Btn disabled={busy} onClick={pullLaunchers}><FolderOpen /> load scripts</Btn>
        </Actions>
        <Field label="script">
          <select value={lName} onChange={(e) => { setLName(e.target.value); setLContent(scripts?.[e.target.value] ?? ""); }}>
            <option value="zw">zw</option>
            <option value="engram">engram</option>
          </select>
        </Field>
        <Field label="content"><textarea value={lContent} onChange={(e) => setLContent(e.target.value)} placeholder="#!/bin/sh …" /></Field>
        <p className="muted">stored: {scripts ? `zw ${scripts.zw ? "yes" : "no"} · engram ${scripts.engram ? "yes" : "no"}` : "none"}</p>
        <Actions>
          <Btn disabled={busy} onClick={saveLauncher}><Save /> save {lName}</Btn>
          <Btn disabled={busy} className="bad" onClick={delLauncher}><Trash2 /> delete {lName}</Btn>
        </Actions>
      </Section>

      <Msg m={m} />
    </Page>
  );
}
