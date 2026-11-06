import { useMemo, useState } from "react";
import qrcode from "qrcode-generator";
import { Check, Copy, GitPullRequest, HeartHandshake, Info, Star } from "lucide-react";
import { Panel, Badge } from "../components/ui";
import { CONTRIBUTION } from "../contribution";
import { useI18n } from "../i18n";
import { pixPayload } from "../pix";

function QrCode({ value, label }: { value: string; label: string }) {
  const modules = useMemo(() => {
    const qr = qrcode(0, "M");
    qr.addData(value);
    qr.make();
    const size = qr.getModuleCount();
    const cells: [number, number][] = [];
    for (let row = 0; row < size; row++) {
      for (let col = 0; col < size; col++) {
        if (qr.isDark(row, col)) cells.push([col, row]);
      }
    }
    return { size, cells };
  }, [value]);
  const quiet = 4;
  const total = modules.size + quiet * 2;
  return (
    <svg
      className="qr"
      viewBox={`0 0 ${total} ${total}`}
      role="img"
      aria-label={label}
      shapeRendering="crispEdges"
    >
      <rect width={total} height={total} fill="#ffffff" />
      <path
        fill="#000000"
        d={modules.cells.map(([x, y]) => `M${x + quiet} ${y + quiet}h1v1h-1z`).join("")}
      />
    </svg>
  );
}

export function Contribute() {
  const { t } = useI18n();
  const [amount, setAmount] = useState<number | undefined>();
  const [custom, setCustom] = useState("");
  const [copied, setCopied] = useState<"pix" | "repo" | null>(null);
  const configured = !!CONTRIBUTION.pixKey && !!CONTRIBUTION.city;

  const payload = useMemo(() => {
    if (!configured) return null;
    try {
      return pixPayload({
        key: CONTRIBUTION.pixKey,
        name: CONTRIBUTION.receiverName,
        city: CONTRIBUTION.city,
        amount,
      });
    } catch {
      return null;
    }
  }, [configured, amount]);

  async function copy(kind: "pix" | "repo", text: string) {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(kind);
      window.setTimeout(() => setCopied(null), 2000);
    } catch {
      setCopied(null);
    }
  }

  function chooseCustom(value: string) {
    const clean = value.replace(/[^\d,]/g, "");
    setCustom(clean);
    const parsed = Number(clean.replace(",", "."));
    setAmount(parsed > 0 && parsed < 100000 ? Math.round(parsed * 100) / 100 : undefined);
  }

  return (
    <>
      <div className="contribute-grid">
        <Panel
          title={t("contribute.pix.title")}
          eyebrow={t("contribute.pix.eyebrow")}
          action={<HeartHandshake size={20} aria-hidden="true" />}
        >
          <p className="section-description">
            {t("contribute.pix.body")}
          </p>
          <p className="field-hint">{t("contribute.pix.what")}</p>
          {payload ? (
            <div className="pix-box">
              <div className="qr-wrap">
                <QrCode
                  value={payload}
                  label={t("contribute.qr.aria", {
                    name: CONTRIBUTION.receiverName,
                    amount: amount
                      ? t("contribute.amount.value", { value: amount.toFixed(2) })
                      : t("contribute.amount.free"),
                  })}
                />
              </div>
              <div className="pix-side">
                <p className="field-label">{t("contribute.amount.label")}</p>
                <div className="amounts" role="group" aria-label={t("contribute.amount.group")}>
                  <button
                    type="button"
                    className={`chip ${amount === undefined && !custom ? "selected" : ""}`}
                    onClick={() => {
                      setAmount(undefined);
                      setCustom("");
                    }}
                  >
                    {t("contribute.chip.free")}
                  </button>
                  {CONTRIBUTION.suggestedAmounts.map((value) => (
                    <button
                      key={value}
                      type="button"
                      className={`chip ${amount === value && !custom ? "selected" : ""}`}
                      onClick={() => {
                        setAmount(value);
                        setCustom("");
                      }}
                    >
                      R$ {value}
                    </button>
                  ))}
                  <input
                    className="chip-input"
                    inputMode="decimal"
                    aria-label={t("contribute.custom.aria")}
                    placeholder={t("contribute.custom.placeholder")}
                    value={custom}
                    onChange={(e) => chooseCustom(e.target.value)}
                  />
                </div>
                <ol className="pix-steps">
                  <li>{t("contribute.step1")}</li>
                  <li>{t("contribute.step2")}</li>
                  <li>{t("contribute.step3")}</li>
                </ol>
                <p className="field-hint">
                  {t("contribute.receiver", { name: CONTRIBUTION.receiverName })}
                </p>
                <button
                  type="button"
                  className="button primary"
                  onClick={() => void copy("pix", payload)}
                >
                  {copied === "pix" ? <Check size={16} aria-hidden="true" /> : <Copy size={16} aria-hidden="true" />}
                  {copied === "pix" ? t("contribute.copied.pix") : t("contribute.copy.pix")}
                </button>
              </div>
            </div>
          ) : (
            <div className="notice warning" role="status">
              <Info size={20} aria-hidden="true" />
              <div>
                <h3>{t("contribute.unconfigured.title")}</h3>
                <p>{t("contribute.unconfigured.body")}</p>
              </div>
            </div>
          )}
          <p className="card-note">
            {t("contribute.note")}
          </p>
        </Panel>
        <Panel title={t("contribute.other.title")} eyebrow={t("contribute.other.eyebrow")}>
          <ul className="help-list">
            <li>
              <Star size={18} aria-hidden="true" />
              <div>
                <strong>{t("contribute.star.title")}</strong>
                <p>{t("contribute.star.body")}</p>
              </div>
            </li>
            <li>
              <GitPullRequest size={18} aria-hidden="true" />
              <div>
                <strong>{t("contribute.pr.title")}</strong>
                <p>{t("contribute.pr.body")}</p>
              </div>
            </li>
          </ul>
          <p className="field-label">{t("contribute.repository")}</p>
          <p className="command">
            <code>{CONTRIBUTION.repository}</code>
            <button type="button" className="link-button" onClick={() => void copy("repo", CONTRIBUTION.repository)}>
              <Copy size={13} aria-hidden="true" /> {copied === "repo" ? t("logs.copied") : t("console.copy")}
            </button>
          </p>
          <Badge tone="blue">{t("contribute.license")}</Badge>
        </Panel>
      </div>
    </>
  );
}
