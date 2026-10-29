import React from "react";
import { api } from "../api";
import { route } from "../app";
import { ArrowLeft, RefreshCw, Play } from "lucide-react";

// Fullscreen tenant app runner: the wrapper's apps area rendered inside the
// UI. One slim top bar with a single control: back to dashboard.
export function AppsFrame() {
  const [src, setSrc] = React.useState<string>("");
  const [err, setErr] = React.useState<string>("");
  const [nonce, setNonce] = React.useState(0);

  React.useEffect(() => {
    (async () => {
      try {
        const r = await api.me();
        const u = r.data?.apps_url ?? localStorage.getItem("wrap.apps_url") ?? "";
        setSrc(u);
        setErr(u ? "" : "no apps area configured for this account");
      } catch {
        setErr("could not load your apps address, going back");
        setTimeout(() => route("#/"), 1500);
      }
    })();
  }, []);

  return (
    <div className="appsview">
      <div className="appsbar">
        <button className="btn" onClick={() => route("#/")}>
          <ArrowLeft /> dashboard
        </button>
        <span className="appsbar-title">
          <Play /> apps
        </span>
        <div className="spacer" />
        <button className="btn bare" title="reload frame" onClick={() => setNonce((n) => n + 1)}>
          <RefreshCw />
        </button>
      </div>
      {err ? (
        <div className="appsview-msg">{err}</div>
      ) : (
        <iframe
          key={nonce}
          className="appsframe"
          src={src}
          title="apps"
          allow="fullscreen"
          referrerPolicy="no-referrer"
        />
      )}
    </div>
  );
}
