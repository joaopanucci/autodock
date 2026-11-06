import { useMemo, useState } from "react";
import qrcode from "qrcode-generator";
import { Check, Copy, GitPullRequest, HeartHandshake, Info, Star } from "lucide-react";
import { Panel, Badge } from "../components/ui";
import { CONTRIBUTION } from "../contribution";
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
          title="Contribua com um PIX"
          eyebrow="APOIE O PROJETO"
          action={<HeartHandshake size={20} aria-hidden="true" />}
        >
          <p className="section-description">
            O AutoDock é open source (Apache-2.0) e gratuito. Se ele economiza o seu tempo, uma
            contribuição voluntária ajuda a manter e evoluir o projeto. Nenhum recurso depende
            dela.
          </p>
          {payload ? (
            <div className="pix-box">
              <div className="qr-wrap">
                <QrCode
                  value={payload}
                  label={`QR Code PIX para ${CONTRIBUTION.receiverName}${amount ? `, valor R$ ${amount.toFixed(2)}` : ", valor livre"}`}
                />
              </div>
              <div className="pix-side">
                <p className="field-label">Valor da contribuição</p>
                <div className="amounts" role="group" aria-label="Valor">
                  <button
                    type="button"
                    className={`chip ${amount === undefined && !custom ? "selected" : ""}`}
                    onClick={() => {
                      setAmount(undefined);
                      setCustom("");
                    }}
                  >
                    Valor livre
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
                    aria-label="Outro valor em reais"
                    placeholder="Outro valor"
                    value={custom}
                    onChange={(e) => chooseCustom(e.target.value)}
                  />
                </div>
                <ol className="pix-steps">
                  <li>Abra o app do seu banco e escolha pagar com PIX.</li>
                  <li>Escaneie o QR Code ao lado, ou use o PIX copia e cola.</li>
                  <li>Confira o recebedor e confirme.</li>
                </ol>
                <p className="field-hint">
                  Recebedor: <strong>{CONTRIBUTION.receiverName}</strong>
                </p>
                <button
                  type="button"
                  className="button primary"
                  onClick={() => void copy("pix", payload)}
                >
                  {copied === "pix" ? <Check size={16} aria-hidden="true" /> : <Copy size={16} aria-hidden="true" />}
                  {copied === "pix" ? "Código copiado" : "Copiar PIX copia e cola"}
                </button>
              </div>
            </div>
          ) : (
            <div className="notice warning" role="status">
              <Info size={20} aria-hidden="true" />
              <div>
                <h3>Contribuição ainda não configurada</h3>
                <p>
                  Para ativar o QR Code, preencha <code>pixKey</code> e <code>city</code> em{" "}
                  <code>apps/desktop/src/contribution.ts</code>.
                </p>
              </div>
            </div>
          )}
          <p className="card-note">
            O QR Code é gerado aqui, no seu computador, sem enviar dados a nenhum serviço. Confira
            sempre o nome do recebedor no app do banco antes de confirmar.
          </p>
        </Panel>
        <Panel title="Outras formas de ajudar" eyebrow="NÃO PRECISA SER EM DINHEIRO">
          <ul className="help-list">
            <li>
              <Star size={18} aria-hidden="true" />
              <div>
                <strong>Dê uma estrela no repositório</strong>
                <p>Ajuda outras pessoas a encontrarem o AutoDock.</p>
              </div>
            </li>
            <li>
              <GitPullRequest size={18} aria-hidden="true" />
              <div>
                <strong>Envie melhorias e relate problemas</strong>
                <p>Issues e pull requests são muito bem-vindos. Veja o CONTRIBUTING.md.</p>
              </div>
            </li>
          </ul>
          <p className="field-label">Repositório</p>
          <p className="command">
            <code>{CONTRIBUTION.repository}</code>
            <button type="button" className="link-button" onClick={() => void copy("repo", CONTRIBUTION.repository)}>
              <Copy size={13} aria-hidden="true" /> {copied === "repo" ? "Copiado" : "Copiar"}
            </button>
          </p>
          <Badge tone="blue">Licença Apache-2.0</Badge>
        </Panel>
      </div>
    </>
  );
}
