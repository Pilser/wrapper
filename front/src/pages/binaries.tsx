import React from "react";
import { api, BUNDLES, sha256Hex, fmtBytes } from "../api";
import { GitCompare, Merge, History, Undo2, Rocket, CloudDownload, Search, Upload } from "lucide-react";
import { Page, Section, Field, Actions, Btn, Msg, errMsg } from "../ui";

export function AdminBinaries() {
  const [list, setList] = React.useState<any>(null);
  const [tenants, setTenants] = React.useState<any[]>([]);
  const [versions, setVersions] = React.useState<any>(null);
  const [vName, setVName] = React.useState<string>(BUNDLES[0]);
  const [diff, setDiff] = React.useState<any | null>(null);
  const [diffName, setDiffName] = React.useState<string>(BUNDLES[0]);
  const [upPath, setUpPath] = React.useState("");
  const [upStage, setUpStage] = React.useState("0");
  const [pushSel, setPushSel] = React.useState("");
  const [syncName, setSyncName] = React.useState<string>(BUNDLES[0]);
  const [syncVer, setSyncVer] = React.useState("");
  const [syncSel, setSyncSel] = React.useState("");
  const [effPull, setEffPull] = React.useState<string>("tenant id");
  const [eff, setEff] = React.useState<any>(null);
  const [m, setM] = React.useState<{ t: "ok" | "bad" | "info"; s: string } | null>(null);
  const [busy, setB] = React.useState(false);

  const load = React.useCallback(async () => {
    try {
      setList((await api.binariesList()).data);
      setTenants((await api.tenantsList()).data.tenants ?? []);
    } catch (e) {
      setM({ t: "bad", s: errMsg(e) });
    }
  }, []);
  React.useEffect(() => { load(); }, [load]);

  async function run(fn: () => Promise<any>, ok: string) {
    setB(true); setM(null);
    try { const r = await fn(); setM({ t: "ok", s: ok + (typeof r?.data === "object" && r?.data ? "" : "") }); await load(); return r; }
    catch (e) { setM({ t: "bad", s: errMsg(e) }); }
    finally { setB(false); }
  }

  async function loadVersions(name: string) {
    setVName(name);
    try { setVersions((await api.binaryVersions(name)).data); } catch (e) { setM({ t: "bad", s: errMsg(e) }); }
  }

  async function upload(file: File) {
    setB(true); setM(null);
    try {
      const buf = await file.arrayBuffer();
      const sha = await sha256Hex(buf);
      const rel = upPath.replace(file.name, "") ? upPath : file.name;
      const path = upPath.trim() || file.name;
      await api.binaryUpload(path, buf, upStage === "1", sha);
      setM({ t: "ok", s: `uploaded ${path}${upStage === "1" ? " (staged)" : " (live)"} (checksum verified)` });
      await load();
    } catch (e) {
      setM({ t: "bad", s: errMsg(e) });
    } finally {
      setB(false);
    }
  }

  return (
    <Page title="BINARIES" sub="agent programs, versioned">
      <Section title="host working set">
        <table>
          <thead><tr><th>path</th><th>size</th><th>sha256</th></tr></thead>
          <tbody>
            {(list?.files ?? []).map((f: any) => (
              <tr key={f.path}>
                <td className="mono">{f.path}</td>
                <td className="num">{fmtBytes(f.size)}</td>
                <td className="muted mono">{String(f.sha256).slice(0, 16)}…</td>
              </tr>
            ))}
          </tbody>
        </table>
      </Section>

      <Section title="upload">
        <Field label="target path"><input value={upPath} onChange={(e) => setUpPath(e.target.value)} placeholder="zw-assets/mytool/v1.2/commands" /></Field>
        <Field label="mode">
          <select value={upStage} onChange={(e) => setUpStage(e.target.value)}>
            <option value="0">live (instant)</option>
            <option value="1">staged (.incoming, diff+merge)</option>
          </select>
        </Field>
        <Field label="file"><input type="file" onChange={(e) => { const f = e.target.files?.[0]; if (f) upload(f); }} /></Field>
        {busy ? <p className="muted">uploading…</p> : null}
        <p className="muted">folders stage first; single files land live directly.</p>
      </Section>

      <Section title="staged diff / merge">
        <Field label="bundle">
          <select value={diffName} onChange={(e) => setDiffName(e.target.value)}>
            {BUNDLES.map((b) => <option key={b} value={b}>{b}</option>)}
          </select>
        </Field>
        <Actions>
          <Btn disabled={busy} onClick={async () => {
            const r = await run(() => api.binaryDiff(diffName), "diff computed");
            setDiff(r?.data ?? null);
          }}><GitCompare /> diff</Btn>
          <Btn disabled={busy} onClick={async () => {
            if (!confirm(`merge staged ${diffName} over live and snapshot a new version?`)) return;
            await run(() => api.binaryMerge(diffName), "merged");
            setDiff(null);
            loadVersions(diffName);
          }}><Merge /> merge</Btn>
        </Actions>
        {diff ? (
          <table>
            <thead><tr><th>state</th><th>path</th><th>detail</th></tr></thead>
            <tbody>
              {[["added", diff.added], ["removed", diff.removed], ["changed", diff.changed], ["unchanged", diff.unchanged]]
                .flatMap(([state, arr]: any) => (arr ?? []).map((x: any) => (
                  <tr key={state + x.path}>
                    <td className={state === "removed" ? "bad" : state === "changed" ? "warn" : state === "added" ? "ok" : "muted"}>{state}</td>
                    <td className="mono">{x.path}</td>
                    <td className="muted mono">{x.new_sha256 ? `${String(x.old_sha256).slice(0, 10)}… -> ${String(x.new_sha256).slice(0, 10)}…` : String(x.sha256 || "").slice(0, 16) + "…"}</td>
                  </tr>
                )))}
            </tbody>
          </table>
        ) : null}
      </Section>

      <Section title="versions">
        <Field label="bundle">
          <select value={vName} onChange={(e) => loadVersions(e.target.value)}>
            {BUNDLES.map((b) => <option key={b} value={b}>{b}</option>)}
          </select>
        </Field>
        {versions ? (
          <p className="muted">
            current:{" "}
            {Object.entries(versions.active ?? {}).length
              ? Object.entries(versions.active).map(([n, v]) => `${n}: version ${v}`).join(" · ")
              : "live working set"}
          </p>
        ) : null}
        {versions ? (
          <table>
            <thead><tr><th>version</th><th>uploaded</th><th>merged</th><th>note</th><th>entries</th><th></th></tr></thead>
            <tbody>
              {(versions.versions ?? []).map((v: any) => (
                <tr key={v.version_no}>
                  <td className="num bold">{v.version_no}</td>
                  <td className="muted">{v.uploaded_at?.slice(0, 19)}</td>
                  <td className="muted">{v.merged_at?.slice(0, 19)}</td>
                  <td>{v.note ?? "none"}</td>
                  <td className="num">{(v.manifest ?? []).length}</td>
                  <td><Btn disabled={busy} onClick={() => {
                    if (!confirm(`rollback ${vName} to v${v.version_no}? restores snapshot + moves pointer`)) return;
                    run(() => api.binaryRollback(vName, v.version_no), `rolled back to v${v.version_no}`).then(() => loadVersions(vName));
                  }}><Undo2 /> restore</Btn></td>
                </tr>
              ))}
            </tbody>
          </table>
        ) : (
          <Actions><Btn disabled={busy} onClick={() => loadVersions(vName)}><History /> load versions</Btn></Actions>
        )}
      </Section>

      <Section title="push / sync to tenants" note="push forces a full refresh on next boot · sync pins one version to a tenant's disk">
        <Field label="push target">
          <select value={pushSel} onChange={(e) => setPushSel(e.target.value)}>
            <option value="">(select, or use bulk)</option>
            {tenants.map((t) => <option key={t.id} value={t.id}>{t.username}</option>)}
          </select>
        </Field>
        <Actions>
          <Btn disabled={busy || !pushSel} onClick={() => run(() => api.binaryPush(pushSel), "stamp dropped, tenant re-waked")}><Rocket /> push</Btn>
          <Btn disabled={busy} onClick={() => { if (confirm("push-bulk to ALL tenants?")) run(() => api.binaryPushBulk({ all: true }), "bulk push done"); }}><Rocket /> push all</Btn>
        </Actions>
        <Field label="sync bundle">
          <select value={syncName} onChange={(e) => setSyncName(e.target.value)}>
            {BUNDLES.map((b) => <option key={b} value={b}>{b}</option>)}
          </select>
        </Field>
        <Field label="version no"><input type="number" value={syncVer} onChange={(e) => setSyncVer(e.target.value)} placeholder="e.g. 3" /></Field>
        <Field label="sync target">
          <select value={syncSel} onChange={(e) => setSyncSel(e.target.value)}>
            <option value="">(select tenant)</option>
            {tenants.map((t) => <option key={t.id} value={t.id}>{t.username}</option>)}
          </select>
        </Field>
        <Actions>
          <Btn disabled={busy || !syncSel || !syncVer} onClick={() =>
            run(() => api.binarySync(syncSel, syncName, +syncVer), "version synced to tenant disk")
          }><CloudDownload /> sync</Btn>
          <Btn disabled={busy || !syncVer} onClick={() => {
            if (confirm(`sync-bulk ${syncName} v${syncVer} to ALL tenants?`))
              run(() => api.binarySyncBulk({ name: syncName, version_no: +syncVer, all: true }), "bulk sync done");
          }}><CloudDownload /> sync all</Btn>
        </Actions>
      </Section>

      <Section title="effective">
        <Field label="tenant id">
          <select value={effPull} onChange={(e) => setEffPull(e.target.value)}>
            <option value="tenant id">(select)</option>
            {tenants.map((t) => <option key={t.id} value={t.id}>{t.username}</option>)}
          </select>
        </Field>
        <Actions><Btn disabled={busy || effPull === "tenant id"} onClick={async () => {
          const r = await run(() => api.binaryEffective(effPull), "resolved");
          setEff(r?.data ?? null);
        }}><Search /> resolve</Btn></Actions>
        {eff ? (
          <div>
            {Object.entries(eff.bundles ?? {}).map(([k, v]: any) => (
              <div className="rowline" key={k}><b>{k}</b><span className="mono">{JSON.stringify(v)}</span></div>
            ))}
          </div>
        ) : null}
      </Section>

      <Msg m={m} />
    </Page>
  );
}
