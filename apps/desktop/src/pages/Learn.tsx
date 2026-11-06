import { useEffect, useState } from "react";
import { ArrowLeft, Check, CheckCircle2, Clock, Copy, Lightbulb, TriangleAlert } from "lucide-react";
import { Badge, Panel } from "../components/ui";
import { useI18n } from "../i18n";
import { LESSONS, type Block, type Lesson } from "../learn/lessons";
import type { Page } from "../types";

const STORAGE_KEY = "autodock.learn.done";

function loadDone(): string[] {
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    const parsed = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed)
      ? parsed.filter((id): id is string => typeof id === "string")
      : [];
  } catch {
    return [];
  }
}

function saveDone(ids: string[]) {
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(ids));
  } catch {
    // Storage can be unavailable; progress then lasts only for this session.
  }
}

export function Learn({ navigate }: { navigate: (page: Page) => void }) {
  const { t, lang } = useI18n();
  const [done, setDone] = useState<string[]>(loadDone);
  const [openId, setOpenId] = useState<string | null>(null);
  const [copied, setCopied] = useState("");

  useEffect(() => saveDone(done), [done]);

  const lesson = LESSONS.find((item) => item.id === openId) ?? null;

  const toggle = (id: string) =>
    setDone((current) =>
      current.includes(id) ? current.filter((item) => item !== id) : [...current, id],
    );

  async function copy(key: string, text: string) {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(key);
      window.setTimeout(() => setCopied((now) => (now === key ? "" : now)), 1800);
    } catch {
      // Clipboard can be blocked; the text stays visible to select by hand.
    }
  }

  const levelLabel = (item: Lesson) => t(`learn.level.${item.level}`);

  function renderBlock(block: Block, index: number) {
    switch (block.kind) {
      case "p":
        return <p key={index}>{block.text[lang]}</p>;
      case "tip":
      case "warn": {
        const Icon = block.kind === "tip" ? Lightbulb : TriangleAlert;
        return (
          <div key={index} className={`learn-callout learn-${block.kind}`}>
            <Icon size={18} aria-hidden="true" />
            <p>{block.text[lang]}</p>
          </div>
        );
      }
      case "code":
        return (
          <figure key={index} className="learn-code">
            {block.caption && <figcaption>{block.caption[lang]}</figcaption>}
            <pre><code>{block.code}</code></pre>
            <button
              type="button"
              className="link-button"
              onClick={() => void copy(`code-${index}`, block.code)}
            >
              {copied === `code-${index}` ? <Check size={14} aria-hidden="true" /> : <Copy size={14} aria-hidden="true" />}
              {copied === `code-${index}` ? t("learn.copied") : t("learn.copyCommand")}
            </button>
          </figure>
        );
    }
  }

  if (lesson) {
    const finished = done.includes(lesson.id);
    return (
      <div className="learn">
        <button type="button" className="link-button" onClick={() => setOpenId(null)}>
          <ArrowLeft size={16} aria-hidden="true" /> {t("learn.back")}
        </button>
        <Panel
          title={lesson.title[lang]}
          eyebrow={t("learn.eyebrow")}
          action={<Badge tone={finished ? "green" : "blue"}>{levelLabel(lesson)}</Badge>}
        >
          <p className="section-description">{lesson.summary[lang]}</p>
          <div className="learn-body">{lesson.blocks.map(renderBlock)}</div>

          {lesson.tryIn && (
            <button
              type="button"
              className="button primary"
              onClick={() => navigate(lesson.tryIn!.page)}
            >
              {lesson.tryIn.label[lang]}
            </button>
          )}

          <h3 className="learn-subtitle">{t("learn.docs")}</h3>
          <ul className="learn-links">
            {lesson.docs.map((doc) => (
              <li key={doc.url}>
                <strong>{doc.label[lang]}</strong>
                <code>{doc.url}</code>
                <button type="button" className="link-button" onClick={() => void copy(doc.url, doc.url)}>
                  {copied === doc.url ? t("learn.copied") : t("learn.copyLink")}
                </button>
              </li>
            ))}
          </ul>

          <button type="button" className="button" onClick={() => toggle(lesson.id)}>
            <CheckCircle2 size={16} aria-hidden="true" />
            {finished ? t("learn.markUndone") : t("learn.markDone")}
          </button>
        </Panel>
      </div>
    );
  }

  const completed = LESSONS.filter((item) => done.includes(item.id)).length;

  return (
    <div className="learn">
      <Panel title={t("page.learn.title")} eyebrow={t("learn.eyebrow")}>
        <p className="section-description">{t("page.learn.description")}</p>
        <div
          className="learn-progress"
          role="progressbar"
          aria-valuemin={0}
          aria-valuemax={LESSONS.length}
          aria-valuenow={completed}
          aria-label={t("learn.progress", { done: completed, total: LESSONS.length })}
        >
          <span style={{ width: `${(completed / LESSONS.length) * 100}%` }} />
        </div>
        <small>{t("learn.progress", { done: completed, total: LESSONS.length })} · {t("learn.private")}</small>
      </Panel>

      <div className="learn-grid">
        {LESSONS.map((item, index) => {
          const finished = done.includes(item.id);
          return (
            <article key={item.id} className={`learn-card${finished ? " is-done" : ""}`}>
              <div className="learn-card-top">
                <span className="learn-number">{finished ? <Check size={14} aria-hidden="true" /> : index + 1}</span>
                <Badge tone={item.level === "beginner" ? "green" : "amber"}>{levelLabel(item)}</Badge>
                <span className="learn-minutes"><Clock size={13} aria-hidden="true" /> {t("learn.minutes", { n: item.minutes })}</span>
              </div>
              <h3>{item.title[lang]}</h3>
              <p>{item.summary[lang]}</p>
              <button type="button" className="button" onClick={() => setOpenId(item.id)}>
                {finished ? t("learn.review") : t("learn.start")}
                <span className="sr-only"> — {item.title[lang]}</span>
              </button>
            </article>
          );
        })}
      </div>
      <p className="learn-note">{t("learn.nextSteps")}</p>
    </div>
  );
}
