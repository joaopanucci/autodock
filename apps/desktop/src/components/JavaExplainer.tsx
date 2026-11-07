import { CheckCircle2, Hand, ShieldCheck } from "lucide-react";
import { Panel } from "./ui";
import { useI18n, type TranslationKey } from "../i18n";

const JOURNEY = [1, 2, 3, 4, 5] as const;
const AUTO = [1, 2, 3, 4, 5, 6] as const;
const MANUAL = [1, 2, 3, 4] as const;
const SAFE = [1, 2, 3, 4] as const;

/**
 * What the Java upgrade does, said in plain words: the path it walks, what it fixes by
 * itself, what is left for the person and why the project stays safe. Open while no
 * project is chosen, so the first visit explains the tool before asking for a folder.
 */
export function JavaExplainer({ open }: { open: boolean }) {
  const { t } = useI18n();
  const key = (name: string) => `java.how.${name}` as TranslationKey;

  return (
    <Panel title={t(key("title"))} eyebrow={t(key("eyebrow"))} className="java-how">
      <details open={open} data-testid="java-how">
        <summary>{t(key("toggle"))}</summary>
        <p className="section-description">{t(key("intro"))}</p>

        <p className="field-label">{t(key("journey.title"))}</p>
        <ol className="java-journey">
          {JOURNEY.map((step) => (
            <li key={step}>
              <strong>{t(key(`journey.${step}.title`))}</strong>
              <span>{t(key(`journey.${step}.body`))}</span>
            </li>
          ))}
        </ol>
        <p className="field-hint">{t(key("journey.note"))}</p>

        <div className="java-how-grid">
          <div>
            <p className="field-label">
              <CheckCircle2 size={15} aria-hidden="true" /> {t(key("auto.title"))}
            </p>
            <ul className="expo-notes">
              {AUTO.map((item) => (
                <li key={item}>{t(key(`auto.${item}`))}</li>
              ))}
            </ul>
          </div>
          <div>
            <p className="field-label">
              <Hand size={15} aria-hidden="true" /> {t(key("manual.title"))}
            </p>
            <ul className="expo-notes">
              {MANUAL.map((item) => (
                <li key={item}>{t(key(`manual.${item}`))}</li>
              ))}
            </ul>
          </div>
        </div>

        <p className="field-label">
          <ShieldCheck size={15} aria-hidden="true" /> {t(key("safe.title"))}
        </p>
        <ul className="expo-notes">
          {SAFE.map((item) => (
            <li key={item}>{t(key(`safe.${item}`))}</li>
          ))}
        </ul>

        <div className="notice java-example">
          <div>
            <h3>{t(key("example.title"))}</h3>
            <p>{t(key("example.body"))}</p>
          </div>
        </div>
        <p className="field-hint">{t(key("cta"))}</p>
      </details>
    </Panel>
  );
}
